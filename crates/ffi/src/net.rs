use std::{path::PathBuf, sync::{Mutex, OnceLock}};

use crate::tasks::Task;


static REQWEST_CLIENT: OnceLock<Mutex<reqwest::Client>> = OnceLock::new();

fn reqwest_client() -> reqwest::Client {
    REQWEST_CLIENT
        .get_or_init(|| Mutex::new(reqwest::Client::builder().build().unwrap()))
        .lock()
        .expect("reqwest client mutex poisoned")
        .clone()
}

pub fn download_file(url: &str, path: &str) -> Box<TaskFile> {
    let client = reqwest_client();
    let req = client.get(url);
    let path = PathBuf::from(path);
    Task::new(async move |ctx| {
        let mut res = req.send().await?;
        let total_size = res.content_length().unwrap_or(0);
        ctx.set_progress_maximum(total_size);

        let mut file =
            File::create(&path).map_err(|err| FfiError::CreateFile(path.clone(), err))?;

        while let Some(chunk) = res.chunk().await? {
            if ctx.is_cancelled() {
                return Err(FfiError::TaskCancelled);
            }
            file.write_all(&chunk)
                .map_err(|err| FfiError::FileWrite(path.clone(), err))?;
            ctx.update(chunk.len() as u64);
        }

        Ok(path)
    })
}

pub fn download_string(url: &str) -> Box<TaskBytes> {
    let client = reqwest_client();
    let req = client.get(url);
    Task::new(async move |ctx| {
        let mut res = req.send().await?;
        let total_size = res.content_length().unwrap_or(0);
        ctx.set_progress_maximum(total_size);

        let mut buf: Vec<u8> = Vec::new();

        while let Some(chunk) = res.chunk().await? {
            if ctx.is_cancelled() {
                return Err(FfiError::TaskCancelled);
            }
            buf.extend(&chunk);
            ctx.update(chunk.len() as u64);
        }

        Ok(buf)
    })
}
