//! 后台请求线程：egui 是即时模式、主线程不能阻塞，所以把 tokio 运行时和
//! SSE 读取放在独立线程里，通过 channel 把增量回传给界面。

use std::sync::mpsc::Receiver;

use anyhow::{Context, Result};
use decem_core::{ChatRequest, Client};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// 后台回传给界面的事件。
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    /// 整个回复完成，附带完整内容。
    Done {
        /// 请求序号。
        request_id: u64,
        /// 完整回复。
        content: String,
    },
    /// 请求失败，附带可直接展示给用户的原因。
    Failed {
        /// 请求序号。
        request_id: u64,
        /// 失败原因。
        message: String,
    },
}

enum Command {
    /// 发起一次流式对话。
    Chat {
        /// 请求序号。
        request_id: u64,
        /// 请求体。
        request: ChatRequest,
        /// API Key。
        api_key: String,
        /// 自定义接口地址。
        api_url: Option<String>,
    },
}

/// 后台线程句柄。
pub struct Worker {
    commands: UnboundedSender<Command>,
    events: Receiver<WorkerEvent>,
}

impl Worker {
    /// 启动后台线程。
    ///
    /// `ctx` 用来在事件到达时唤醒界面重绘（后台线程不知道帧循环何时空闲）。
    pub fn spawn(ctx: egui::Context) -> Result<Self> {
        let (command_tx, mut command_rx) = unbounded_channel::<Command>();
        let (event_tx, event_rx) = std::sync::mpsc::channel::<WorkerEvent>();

        std::thread::Builder::new()
            .name("decem-worker".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        let _ = event_tx.send(WorkerEvent::Failed {
                            request_id: 0,
                            message: format!("后台运行时启动失败：{err}"),
                        });
                        ctx.request_repaint();
                        return;
                    }
                };

                runtime.block_on(async move {
                    let mut current: Option<tokio::task::JoinHandle<()>> = None;
                    while let Some(command) = command_rx.recv().await {
                        let Command::Chat {
                            request_id,
                            request,
                            api_key,
                            api_url,
                        } = command;

                        // 同一时刻只允许一个请求在跑：新的请求直接顶掉旧的
                        if let Some(handle) = current.take() {
                            handle.abort();
                        }

                        let event_tx = event_tx.clone();
                        let ctx = ctx.clone();
                        current = Some(tokio::spawn(async move {
                            let outcome =
                                stream_chat(&request, &api_key, api_url.as_deref(), &ctx).await;
                            let event = match outcome {
                                Ok(content) => WorkerEvent::Done {
                                    request_id,
                                    content,
                                },
                                Err(err) => WorkerEvent::Failed {
                                    request_id,
                                    message: format!("API 请求失败: {err}"),
                                },
                            };
                            let _ = event_tx.send(event);
                            ctx.request_repaint();
                        }));
                    }
                });
            })
            .context("无法创建后台请求线程")?;

        Ok(Self {
            commands: command_tx,
            events: event_rx,
        })
    }

    /// 发起一次对话；重复调用会取消上一次未完成的请求。
    pub fn start_chat(
        &self,
        request_id: u64,
        request: ChatRequest,
        api_key: String,
        api_url: Option<String>,
    ) -> Result<()> {
        self.commands
            .send(Command::Chat {
                request_id,
                request,
                api_key,
                api_url,
            })
            .map_err(|_| anyhow::anyhow!("后台线程已退出"))
    }

    /// 取走当前已到达的所有事件。
    pub fn drain(&self) -> Vec<WorkerEvent> {
        self.events.try_iter().collect()
    }
}

/// 消费一条 SSE 流，把增量与最终结果通过 channel 发出。
async fn stream_chat(
    request: &ChatRequest,
    api_key: &str,
    api_url: Option<&str>,
    ctx: &egui::Context,
) -> Result<String, decem_core::Error> {
    let mut client = Client::new(api_key)?;
    if let Some(url) = api_url.filter(|url| !url.trim().is_empty()) {
        client = client.with_api_url(url);
    }

    // 逐块读完整条流；界面只关心最终结果（与网页版一致：收完再逐字显示）
    let mut stream = client.open_stream(request).await?;
    while stream.next_chunk().await?.is_some() {
        ctx.request_repaint();
    }
    Ok(stream.content().to_owned())
}
