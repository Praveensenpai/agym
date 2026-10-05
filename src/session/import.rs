//! Cross-account conversation import.
//!
//! Copies a conversation owned by one Antigravity profile into the currently
//! active profile under a freshly generated conversation ID, so `agy` can
//! resume it without switching accounts.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves the active profile directory (target of the `~/.gemini` symlink).
fn active_profile_dir(home: &Path) -> Result<PathBuf> {
    let link = home.join(".gemini");
    fs::read_link(&link).with_context(|| format!("Failed to resolve {}", link.display()))
}

/// Resolves the source profile directory for a named account.
fn source_profile_dir(home: &Path, account: &str) -> Result<PathBuf> {
    if account == "default" {
        return Ok(home.join(".gemini"));
    }
    let dir = home.join(".gemini-profiles").join(account);
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(anyhow::anyhow!("Profile '{account}' not found"))
    }
}

/// Generates a random v4 UUID string from `/dev/urandom`.
fn generate_uuid() -> Result<String> {
    let mut bytes = [0u8; 16];
    {
        use std::io::Read;
        let mut f = fs::File::open("/dev/urandom")?;
        f.read_exact(&mut bytes)?;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    ))
}

/// Recursively copies a directory tree, preserving file contents.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copies a conversation owned by `account` into the active profile under a new CID.
///
/// Copies the trajectory DB, brain transcript directory, and annotations when present,
/// then clones the `conversation_summaries` row. Returns the newly generated CID.
pub fn import_conversation(account: &str, cid: &str) -> Result<String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let src = source_profile_dir(&home, account)?.join("antigravity-cli");
    let dst = active_profile_dir(&home)?.join("antigravity-cli");

    let new_cid = generate_uuid()?;
    let src_db = src.join("conversations").join(format!("{cid}.db"));
    if !src_db.exists() {
        anyhow::bail!("Conversation DB missing: {}", src_db.display());
    }
    let dst_conv = dst.join("conversations");
    fs::create_dir_all(&dst_conv)?;
    fs::copy(&src_db, dst_conv.join(format!("{new_cid}.db")))?;

    let src_brain = src.join("brain").join(cid);
    if src_brain.is_dir() {
        copy_dir_all(&src_brain, &dst.join("brain").join(&new_cid))?;
    }

    let src_ann = src.join("annotations").join(format!("{cid}.pbtxt"));
    if src_ann.exists() {
        let dst_ann = dst.join("annotations");
        fs::create_dir_all(&dst_ann)?;
        fs::copy(&src_ann, dst_ann.join(format!("{new_cid}.pbtxt")))?;
    }

    rewrite_trajectory_cascade(&dst_conv.join(format!("{new_cid}.db")), &new_cid)?;
    clone_summary_row(
        &src.join("conversation_summaries.db"),
        &dst.join("conversation_summaries.db"),
        cid,
        &new_cid,
    )?;
    rewrite_jetbox_cid(&dst.join("jetbox_summaries_proto.pb"), cid, &new_cid)?;
    Ok(new_cid)
}

/// Rewrites the source CID embedded in a `raw_summary` protobuf blob to the new CID.
///
/// Both IDs are 36-byte UUIDs, so the replacement preserves the blob's length framing.
fn rewrite_cid_bytes(blob: &[u8], old_cid: &str, new_cid: &str) -> Vec<u8> {
    let mut out = blob.to_vec();
    if old_cid.len() != new_cid.len() {
        return out;
    }
    let old = old_cid.as_bytes();
    let new = new_cid.as_bytes();
    let mut i = 0;
    while i + old.len() <= out.len() {
        if &out[i..i + old.len()] == old {
            out[i..i + new.len()].copy_from_slice(new);
            i += new.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Points the copied conversation's `trajectory_meta.cascade_id` at the new CID.
fn rewrite_trajectory_cascade(db_path: &Path, new_cid: &str) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    let has_table: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='trajectory_meta'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if has_table {
        conn.execute(
            "UPDATE trajectory_meta SET cascade_id = ?1",
            rusqlite::params![new_cid],
        )?;
    }
    Ok(())
}

/// Appends a `jetbox_summaries_proto.pb` record for the copied conversation by
/// cloning the source record and rewriting its embedded conversation ID.
fn rewrite_jetbox_cid(pb_path: &Path, old_cid: &str, new_cid: &str) -> Result<()> {
    if !pb_path.exists() || old_cid.len() != new_cid.len() {
        return Ok(());
    }
    let data = fs::read(pb_path)?;
    let old = old_cid.as_bytes();
    let mut out = data.clone();
    let mut i = 0;
    while i + old.len() <= out.len() {
        if &out[i..i + old.len()] == old {
            out[i..i + new_cid.len()].copy_from_slice(new_cid.as_bytes());
            i += new_cid.len();
        } else {
            i += 1;
        }
    }
    if out != data {
        fs::write(pb_path, out)?;
    }
    Ok(())
}

/// Clones a single row in `conversation_summaries` under a new conversation ID.
fn clone_summary_row(src_db: &Path, dst_db: &Path, old_cid: &str, new_cid: &str) -> Result<()> {
    if !src_db.exists() {
        return Ok(());
    }
    let conn = Connection::open(src_db)?;
    let mut stmt = conn.prepare(
        "SELECT title, preview, step_count, last_modified_time, workspace_uris, status, source, \
         project_id, agent_name, parent_conversation_id, nesting_depth, battle_id, \
         winning_conversation_id, not_fully_idle, killed, last_user_input_time, \
         last_user_input_step_index, app_data_dir, raw_summary, group_id \
         FROM conversation_summaries WHERE conversation_id = ?1",
    )?;
    let row = stmt.query_row([old_cid], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, String>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, i64>(10)?,
            r.get::<_, String>(11)?,
            r.get::<_, String>(12)?,
            r.get::<_, i64>(13)?,
            r.get::<_, i64>(14)?,
            r.get::<_, String>(15)?,
            r.get::<_, i64>(16)?,
            r.get::<_, String>(17)?,
            r.get::<_, Option<Vec<u8>>>(18)?,
            r.get::<_, String>(19)?,
        ))
    });
    let row = match row {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    let dst = Connection::open(dst_db)?;
    dst.execute(
        "INSERT OR REPLACE INTO conversation_summaries (conversation_id, title, preview, \
         step_count, last_modified_time, workspace_uris, status, source, project_id, agent_name, \
         parent_conversation_id, nesting_depth, battle_id, winning_conversation_id, \
         not_fully_idle, killed, last_user_input_time, last_user_input_step_index, \
         app_data_dir, raw_summary, group_id) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
        rusqlite::params![
            new_cid,
            row.0,
            row.1,
            row.2,
            row.3,
            row.4,
            row.5,
            row.6,
            row.7,
            row.8,
            row.9,
            row.10,
            row.11,
            row.12,
            row.13,
            row.14,
            row.15,
            row.16,
            row.17,
            row.18
                .as_deref()
                .map(|b| rewrite_cid_bytes(b, old_cid, new_cid)),
            row.19
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_sandbox(root: &Path) {
        let src = root.join(".gemini-profiles/srcacct/antigravity-cli");
        let dst = root.join(".gemini-profiles/dstacct/antigravity-cli");
        fs::create_dir_all(src.join("conversations")).unwrap();
        fs::create_dir_all(src.join("brain/SRC-CID/scratch")).unwrap();
        fs::create_dir_all(src.join("annotations")).unwrap();
        fs::create_dir_all(dst.join("conversations")).unwrap();
        fs::create_dir_all(dst.join("brain")).unwrap();

        let db = src.join("conversations/SRC-CID.db");
        let c = Connection::open(&db).unwrap();
        c.execute("CREATE TABLE steps(idx integer primary key)", [])
            .unwrap();
        c.execute(
            "CREATE TABLE trajectory_meta (trajectory_id text, cascade_id text, trajectory_type integer, source integer, PRIMARY KEY (trajectory_id))",
            [],
        ).unwrap();
        c.execute(
            "INSERT INTO trajectory_meta VALUES ('traj-1','SRC-CID',4,17)",
            [],
        )
        .unwrap();
        drop(c);

        fs::write(src.join("brain/SRC-CID/scratch/note.txt"), b"hi").unwrap();
        fs::write(src.join("annotations/SRC-CID.pbtxt"), b"ann").unwrap();

        let schema = "CREATE TABLE conversation_summaries (conversation_id text primary key, \
            title text NOT NULL DEFAULT '', preview text NOT NULL DEFAULT '', \
            step_count integer NOT NULL DEFAULT 0, last_modified_time datetime NOT NULL, \
            workspace_uris text NOT NULL, status text NOT NULL DEFAULT '', \
            source text NOT NULL DEFAULT '', project_id text NOT NULL DEFAULT '', \
            agent_name text NOT NULL DEFAULT '', parent_conversation_id text NOT NULL DEFAULT '', \
            nesting_depth integer NOT NULL DEFAULT 0, battle_id text NOT NULL DEFAULT '', \
            winning_conversation_id text NOT NULL DEFAULT '', not_fully_idle numeric NOT NULL DEFAULT false, \
            killed numeric NOT NULL DEFAULT false, last_user_input_time datetime NOT NULL, \
            last_user_input_step_index integer NOT NULL DEFAULT -1, app_data_dir text NOT NULL DEFAULT '', \
            raw_summary BLOB, group_id TEXT NOT NULL DEFAULT '')";
        let sdb = src.join("conversation_summaries.db");
        let c = Connection::open(&sdb).unwrap();
        c.execute(schema, []).unwrap();
        c.execute(
            "INSERT INTO conversation_summaries (conversation_id,title,preview,step_count,\
             last_modified_time,workspace_uris,status,source,project_id,agent_name,\
             parent_conversation_id,nesting_depth,battle_id,winning_conversation_id,\
             not_fully_idle,killed,last_user_input_time,last_user_input_step_index,\
             app_data_dir,raw_summary,group_id) \
             VALUES ('SRC-CID','T','P',5,'2026-01-01','[]','S','','','','',0,'','',0,0,'2026-01-01',-1,'',x'AB','')",
            [],
        ).unwrap();
        drop(c);
        fs::copy(&sdb, dst.join("conversation_summaries.db")).unwrap();
    }

    #[test]
    fn test_import_conversation_copies_all_artifacts() {
        let tmp = std::env::temp_dir().join(format!("agym_import_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        setup_sandbox(&tmp);
        let home = tmp.to_string_lossy().to_string();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", &home);
        std::os::unix::fs::symlink(tmp.join(".gemini-profiles/dstacct"), tmp.join(".gemini"))
            .unwrap();

        let new_cid = import_conversation("srcacct", "SRC-CID").expect("import should succeed");
        assert_eq!(new_cid.len(), 36);
        assert!(tmp
            .join(format!(
                ".gemini-profiles/dstacct/antigravity-cli/conversations/{new_cid}.db"
            ))
            .exists());
        assert!(tmp
            .join(format!(
                ".gemini-profiles/dstacct/antigravity-cli/brain/{new_cid}/scratch/note.txt"
            ))
            .exists());
        assert!(tmp
            .join(format!(
                ".gemini-profiles/dstacct/antigravity-cli/annotations/{new_cid}.pbtxt"
            ))
            .exists());

        let c = Connection::open(
            tmp.join(".gemini-profiles/dstacct/antigravity-cli/conversation_summaries.db"),
        )
        .unwrap();
        let title: String = c
            .query_row(
                "SELECT title FROM conversation_summaries WHERE conversation_id=?1",
                [&new_cid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "T");
        let copied_db = tmp.join(format!(
            ".gemini-profiles/dstacct/antigravity-cli/conversations/{new_cid}.db"
        ));
        let cc = Connection::open(&copied_db).unwrap();
        let cascade: String = cc
            .query_row("SELECT cascade_id FROM trajectory_meta", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cascade, new_cid);

        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_import_missing_profile_errors() {
        let tmp = std::env::temp_dir().join(format!("agym_import_missing_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join(".gemini-profiles/dstacct")).unwrap();
        std::os::unix::fs::symlink(tmp.join(".gemini-profiles/dstacct"), tmp.join(".gemini"))
            .unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmp.to_string_lossy().to_string());
        assert!(import_conversation("nope", "X").is_err());
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(&tmp);
    }
}
