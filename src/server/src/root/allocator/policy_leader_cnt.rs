// Copyright 2022 The Engula Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use engula_api::server::v1::{NodeDesc, RaftRole, ReplicaDesc, ReplicaRole};
use tracing::debug;

use super::{source::NodeFilter, AllocSource, BalanceStatus, LeaderAction, TransferLeader};
use crate::{bootstrap::ROOT_GROUP_ID, Result};

pub struct LeaderCountPolicy<T: AllocSource> {
    alloc_source: Arc<T>,
}

enum TransferDescision {
    TransferOnly {
        group: u64,
        src_replica: u64,
        target_replica: u64,
        src_node: u64,
        target_node: u64,
    },
    // TODO: add create then transfer option?
}

impl<T: AllocSource> LeaderCountPolicy<T> {
    pub fn with(alloc_source: Arc<T>) -> Self {
        Self { alloc_source }
    }

    pub fn compute_balance(&self) -> Result<LeaderAction> {
        let mean = self.mean_leader_count(NodeFilter::Schedulable);
        let candidate_nodes = self.alloc_source.nodes(NodeFilter::Schedulable);
        let ranked_nodes = Self::rank_nodes_for_leader(candidate_nodes, mean);
        debug!(
            scored_nodes = ?ranked_nodes.iter().map(|(n, s)| format!("{}-{}({:?})", n.id, n.capacity.as_ref().unwrap().leader_count, s)).collect::<Vec<_>>(),
            mean = mean,
            "node ranked by leader count",
        );
        for (n, _) in ranked_nodes
            .iter()
            .filter(|(_, s)| *s == BalanceStatus::Overfull)
        {
            if let Some(descision) = self.try_descrease_node_leader_count(n, &ranked_nodes, mean)? {
                match descision {
                    TransferDescision::TransferOnly {
                        group,
                        src_replica,
                        target_replica,
                        src_node,
                        target_node,
                    } => {
                        return Ok(LeaderAction::Shed(TransferLeader {
                            group,
                            src_node,
                            src_replica,
                            target_node,
                            target_replica,
                        }));
                    }
                }
            }
        }
        Ok(LeaderAction::Noop)
    }

    fn try_descrease_node_leader_count(
        &self,
        n: &NodeDesc,
        ranked_nodes: &[(NodeDesc, BalanceStatus)],
        mean: f64,
    ) -> Result<Option<TransferDescision>> {
        let node_replicas = self.alloc_source.node_replicas(&n.id);
        let groups = self.alloc_source.groups();
        for (replica, group_id) in node_replicas
            .iter()
            .filter(|(r, g)| *g != ROOT_GROUP_ID && r.role == ReplicaRole::Voter as i32)
        {
            let replica_state = self.alloc_source.replica_state(&replica.id);
            if replica_state.is_none() {
                // The replica existed in group_desc, but not found in replica_state, the reason(if
                // no code bug) should be: 1. "group_desc update" has be taken
                // effect, but "replica_state update" is delayed(e.g. report net fail and heartbeat
                // still wait next turn)
                //
                // It's very low probability for a not exist replica_state be a leader, so we choose
                // try other replicas here without waiting new replica_state update.
                continue;
            }
            if replica_state.as_ref().unwrap().role != RaftRole::Leader as i32 {
                continue;
            }

            let group = groups
                .get(group_id)
                .expect("group {group_id} inconsistent with node-group index");
            let exist_replica_in_nodes = group
                .replicas
                .iter()
                .filter(|r| r.id != replica.id)
                .map(|r| (r.node_id, r.to_owned()))
                .collect::<HashMap<u64, ReplicaDesc>>();

            for target_node in ranked_nodes
                .iter()
                .rev()
                .filter(|(_, s)| *s == BalanceStatus::Underfull)
                .map(|e| &e.0)
            {
                let sim_count = (target_node.capacity.as_ref().unwrap().leader_count + 1) as f64;
                if Self::leader_balance_state(sim_count, mean) == BalanceStatus::Overfull {
                    continue;
                }
                let target_replica = exist_replica_in_nodes.get(&target_node.id);
                if target_replica.is_none() {
                    continue;
                }
                let target_replica = target_replica.unwrap();
                return Ok(Some(TransferDescision::TransferOnly {
                    group: group_id.to_owned(),
                    src_replica: replica.id,
                    target_replica: target_replica.id,
                    src_node: replica.node_id,
                    target_node: target_replica.node_id,
                }));
            }
        }
        Ok(None)
    }

    fn rank_nodes_for_leader(ns: Vec<NodeDesc>, mean_cnt: f64) -> Vec<(NodeDesc, BalanceStatus)> {
        let mut with_status = ns
            .into_iter()
            .map(|n| {
                let leader_num = n.capacity.as_ref().unwrap().leader_count as f64;
                let s = Self::leader_balance_state(leader_num, mean_cnt);
                (n, s)
            })
            .collect::<Vec<(NodeDesc, BalanceStatus)>>();
        with_status.sort_by(|n1, n2| {
            if (n2.1 == BalanceStatus::Overfull) && (n1.1 != BalanceStatus::Overfull) {
                return Ordering::Greater;
            }
            if (n2.1 == BalanceStatus::Underfull) && (n1.1 != BalanceStatus::Underfull) {
                return Ordering::Less;
            }
            return n2
                .0
                .capacity
                .as_ref()
                .unwrap()
                .leader_count
                .cmp(&n1.0.capacity.as_ref().unwrap().leader_count);
        });
        with_status
    }

    fn leader_balance_state(replica_num: f64, mean: f64) -> BalanceStatus {
        let delta = 0.5;
        if replica_num > mean + delta {
            return BalanceStatus::Overfull;
        }
        if replica_num < mean - delta {
            return BalanceStatus::Underfull;
        }
        BalanceStatus::Balanced
    }

    fn mean_leader_count(&self, filter: NodeFilter) -> f64 {
        let nodes = self.alloc_source.nodes(filter);
        let total_leaders = nodes
            .iter()
            .map(|n| n.capacity.as_ref().unwrap().leader_count)
            .sum::<u64>() as f64;
        total_leaders / (nodes.len() as f64)
    }
}
