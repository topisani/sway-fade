use rand::Rng;
use std::time::Duration;

use async_std::task;
use structopt::StructOpt;
use swayipc_async::{Connection, Fallible};

#[derive(StructOpt, Clone, Debug)]
enum Cmd {
    /// Fade a new window in
    In,
    /// Quit a window by fading it out
    Out,
    /// Switch workspace with crossfade
    Ws {
        /// The name of the workspace to switch to
        name: String,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "sway-fade",
    about = "Fade windows and workspaces in and out in sway"
)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Cmd,
    #[structopt(short, long, default_value = "10")]
    steps: u32,
    #[structopt(short, long, default_value = "0.1")]
    time: f32,
}

struct App {
    opt: Opt,
    sway: Connection,
}

impl App {
    async fn fade_in(&mut self) -> Fallible<()> {
        let id = rand::thread_rng().gen_range(0..9999);
        self.sway
            .run_command(format!("[con_mark=fade] mark {}; unmark fade", id))
            .await?;

        let stride = 1.0 / (self.opt.steps as f32);

        for _ in 0..self.opt.steps {
            task::sleep(Duration::from_secs_f32(
                self.opt.time / (self.opt.steps as f32),
            ))
            .await;
            self.sway
                .run_command(format!("[con_mark={}] opacity plus {}", id, stride))
                .await?;
        }
        self.sway
            .run_command(format!("[con_mark={}] opacity 1", id))
            .await?;
        self.sway
            .run_command(format!("[con_mark={}] unmark {}", id, id))
            .await?;

        Ok(())
    }

    async fn fade_out(&mut self) -> Fallible<()> {
        let id = rand::thread_rng().gen_range(0..9999);
        self.sway
            .run_command(format!("[con_mark=quit] mark {}; unmark quit", id))
            .await?;

        let stride = 1.0 / (self.opt.steps as f32);

        for _ in 0..self.opt.steps {
            task::sleep(Duration::from_secs_f32(
                self.opt.time / (self.opt.steps as f32),
            ))
            .await;
            self.sway
                .run_command(format!("[con_mark={}] opacity minus {}", id, stride))
                .await?;
        }
        self.sway
            .run_command(format!("[con_mark={}] kill", id))
            .await?;

        Ok(())
    }

    async fn fade_ws(&mut self, name: String) -> Fallible<()> {
        let workspaces = self.sway.get_workspaces().await?;
        let new_ws = workspaces.iter().find(|x| x.name == name);
        let output = if let Some(new_ws) = new_ws {
            new_ws.output.clone()
        } else {
            let outputs = self.sway.get_outputs().await?;
            let output = outputs
                .iter()
                .find(|x| x.focused)
                .unwrap_or(outputs.first().expect("Couldn't get any outputs"));
            output.name.clone()
        };
        let cur_ws = workspaces
            .iter()
            .find(|x| x.visible && x.output == output)
            .or_else(|| workspaces.iter().find(|x| x.focused))
            .expect("Couldnt find current workspace");

        if new_ws.filter(|x| x.visible).is_some() {
            // No flashing when the workspace is already visible
            self.sway.run_command(format!("workspace {}", name)).await?;
            return Ok(());
        }

        let stride = 1.0 / (self.opt.steps as f32);
        let wait_time = self.opt.time / (self.opt.steps as f32) / 2.0;

        // Fade old workspace out
        for _ in 0..self.opt.steps {
            self.sway
                .run_command(format!("[workspace={}] opacity minus {}", cur_ws.num, stride))
                .await?;
            task::sleep(Duration::from_secs_f32(wait_time)).await;
        }
        // Make new windows invisible
        self.sway
            .run_command(format!("[workspace={}] opacity 0", name))
            .await?;
        // Switch
        self.sway.run_command(format!("workspace {}", name)).await?;

        // Make windows on old workspace visible
        self.sway
            .run_command(format!("[workspace={}] opacity 1", cur_ws.num))
            .await?;

        // Fade new ws in
        for _ in 0..self.opt.steps {
            self.sway
                .run_command(format!("[workspace=__focused__] opacity plus {}", stride))
                .await?;
            task::sleep(Duration::from_secs_f32(wait_time)).await;
        }

        // Make new windows visible
        self.sway
            .run_command(format!("[workspace=__focused__] opacity 1"))
            .await?;
        
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Fallible<()> {
    let mut app = App {
        opt: Opt::from_args(),
        sway: Connection::new().await?,
    };

    match app.opt.cmd.clone() {
        Cmd::In => app.fade_in().await?,
        Cmd::Out => app.fade_out().await?,
        Cmd::Ws { name } => app.fade_ws(name).await?,
    }

    Ok(())
}
