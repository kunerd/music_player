use std::fs::File;
use std::io::BufReader;
use std::mem;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::Duration;

use iced::widget::{button, center, column, container, keyed_column, row, text};
use iced::Length::Fill;
use iced::{window, Element, Task};
use lofty::file::AudioFile;
use lofty::probe::Probe;
use rodio::{OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod track;
use crate::track::*;

pub const HOME_PATH: &str = FIXME;

fn main() -> iced::Result {
    // Make config with its config file
    let path = PathBuf::from_str(HOME_PATH).unwrap();

    if !path.exists() {
        std::fs::create_dir(path).unwrap();
    }

    iced::application(Player::title, Player::update, Player::view)
        .window(window::Settings {
            ..Default::default()
        })
        .run_with(Player::new)
}

struct Player {
    tracks: Vec<Track>,
    sender: mpsc::Sender<Command>,
}

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Play(PathBuf),
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Vec<Track>, LoadError>),
    TrackMessage(usize, TrackMessage),
    Err(Result<(), String>),
}

impl Player {
    fn new() -> (Self, Task<Message>) {
        let (tx, rx) = mpsc::channel::<Command>();

        tokio::task::spawn_blocking(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();

            while let Ok(command) = rx.recv() {
                match command.clone() {
                    Command::Play(path) => {
                        let file = File::open(path).unwrap();
                        let source = rodio::Decoder::new_mp3(BufReader::new(file)).unwrap();
                        let dur = source.total_duration();
                        println!("Total duration = {dur:#?}");
                        sink.append(source);
                    }
                };

                println!("Currently tracks in queue = {}", sink.len());
                println!("Track is on position = {:?} s", sink.get_pos());
            }

            dbg!("Engine died");
        });

        let player = Player {
            tracks: vec![],
            sender: tx,
        };
        (player, Task::perform(SavedState::load(), Message::Loaded))
    }

    fn title(&self) -> String {
        "Iced music player".to_string()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(Ok(tracks)) => {
                self.tracks = tracks;

                //tokio::spawn(async move {
                //    self.play_track(path)
                //})

                Task::none()
            }
            Message::Loaded(Err(_err)) => Task::none(),
            Message::TrackMessage(i, track_message) => {
                if let Some(track) = self.tracks.get_mut(i) {
                    dbg!(i, &track_message);
                    let _a = track.update(track_message);
                    let path = track.path.clone();
                    let sender = self.sender.clone();

                    Task::perform(
                        async move {
                            let _ = sender.send(Command::Play(path));
                        },
                        |_| (),
                    )
                    .discard()
                } else {
                    Task::none()
                }
            }
            Message::Err(res) => {
                println!("{res:#?}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        // let cont = text("helol");

        let tracks: Element<_> = if self.tracks.len() > 0 {
            keyed_column(self.tracks.iter().enumerate().map(|(i, track)| {
                (
                    track.uuid,
                    track
                        .view()
                        .map(move |message| Message::TrackMessage(i, message)),
                )
            }))
            .spacing(10)
            .height(Fill)
            .into()
        } else {
            center(text("Hello").width(Fill).size(25).color([0.7, 0.7, 0.7]))
                .height(200)
                .into()
        };

        let control =
            container(row![button("<"), button("||"), button(">")].spacing(50)).center_x(Fill);
        let content = column![tracks, control].padding([10, 20]);
        container(content).width(Fill).height(Fill).into()
    }

    //async fn play_track(&self, path: PathBuf) -> Result<(), String> {
    //
    //}
}

#[derive(Debug, Clone)]
pub enum LoadError {
    File,
    Format,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedState {
    tracks: Vec<Track>,
}

impl SavedState {
    pub async fn load() -> Result<Vec<Track>, LoadError> {
        let mut tracks = vec![];
        let mut paths = vec![];
        Self::visit_dir(&mut paths, HOME_PATH.into());

        for path in paths {
            let track_metadata = Probe::open(&path)
                .map_err(|_| LoadError::File)?
                .read()
                .map_err(|_| LoadError::File)?;

            let duration = track_metadata.properties().duration().as_secs();
            let duration = format!("{}:{}", duration / 60, duration % 60);

            tracks.push(Track {
                uuid: Uuid::new_v4(),
                name: path.file_name().unwrap().to_str().unwrap().to_string(),
                duration,
                path,
            });
        }

        Ok(tracks)
    }

    fn visit_dir(paths: &mut Vec<PathBuf>, dir: PathBuf) {
        if dir.is_dir() {
            for entry in dir.read_dir().unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    Self::visit_dir(paths, path);
                } else if path.is_file() && path.extension().unwrap() == "mp3" {
                    paths.push(path);
                }
            }
        }
    }
}
