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
use crate::{
    node::{engine::SnapshotMode, GroupEngine, Replica},
    NodeConfig, Result,
};

pub async fn remove_shard(
    cfg: &NodeConfig,
    replica: &Replica,
    group_engine: GroupEngine,
    shard_id: u64,
) -> Result<()> {
    let mut latest_key: Option<Vec<u8>> = None;
    loop {
        let chunk = collect_chunks(cfg, &group_engine, shard_id, latest_key.as_deref()).await?;
        if chunk.is_empty() {
            break;
        }
        latest_key = Some(chunk.last().unwrap().0.to_owned());
        replica.delete_chunks(shard_id, &chunk).await?;
    }
    Ok(())
}

async fn collect_chunks(
    cfg: &NodeConfig,
    group_engine: &GroupEngine,
    shard_id: u64,
    start_key: Option<&[u8]>,
) -> Result<Vec<(Vec<u8>, u64)>> {
    let snapshot_mode = SnapshotMode::Start { start_key };
    let mut snapshot = group_engine.snapshot(shard_id, snapshot_mode)?;
    let mut buf = Vec::with_capacity(cfg.shard_gc_keys);
    for mvcc_iter in snapshot.iter() {
        let mvcc_iter = mvcc_iter?;
        for entry in mvcc_iter {
            let e = entry?;
            buf.push((e.user_key().to_owned(), e.version()));
        }
        if buf.len() >= cfg.shard_gc_keys {
            break;
        }
    }
    Ok(buf)
}
