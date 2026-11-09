use rodio::{Decoder, DeviceSinkBuilder, Player, Source};
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum AudioPlaybackEvent {
    State {
        position: f64,
        duration: f64,
        is_playing: bool,
    },
    Ended,
    Error(String),
}

enum Command {
    Load(PathBuf),
    SetPlaying(bool),
    Seek(f64),
    SetSpeed(f32),
    SetVolume(f32),
    Stop,
}

#[derive(Clone)]
pub struct AudioPlaybackEvents(Arc<Mutex<Receiver<AudioPlaybackEvent>>>);

impl AudioPlaybackEvents {
    pub fn try_recv(&self) -> Result<AudioPlaybackEvent, TryRecvError> {
        self.0.lock().unwrap().try_recv()
    }
}

/// Thread-safe command handle for the native audio output worker.
pub struct AudioPlaybackController {
    commands: Sender<Command>,
    events: AudioPlaybackEvents,
}

impl AudioPlaybackController {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("sona-audio-playback".into())
            .spawn(move || run_playback_worker(command_rx, event_tx))
            .expect("failed to start audio playback worker");
        Self {
            commands: command_tx,
            events: AudioPlaybackEvents(Arc::new(Mutex::new(event_rx))),
        }
    }

    pub fn events(&self) -> AudioPlaybackEvents {
        self.events.clone()
    }

    pub fn load(&self, path: PathBuf) {
        let _ = self.commands.send(Command::Load(path));
    }

    pub fn set_playing(&self, is_playing: bool) {
        let _ = self.commands.send(Command::SetPlaying(is_playing));
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.commands.send(Command::Seek(seconds.max(0.0)));
    }

    pub fn set_speed(&self, speed: f32) {
        let _ = self
            .commands
            .send(Command::SetSpeed(speed.clamp(0.25, 4.0)));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self
            .commands
            .send(Command::SetVolume(volume.clamp(0.0, 1.0)));
    }

    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
    }
}

impl Default for AudioPlaybackController {
    fn default() -> Self {
        Self::new()
    }
}

fn run_playback_worker(commands: Receiver<Command>, events: Sender<AudioPlaybackEvent>) {
    let mut _stream = None;
    let mut player: Option<Player> = None;
    let mut duration = 0.0;
    let mut previous_position = Duration::MAX;
    let mut was_playing = false;
    let mut ended_sent = false;

    loop {
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Load(path)) => {
                if let Some(player) = &player {
                    player.stop();
                    player.clear();
                }
                ended_sent = false;
                match File::open(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|file| Decoder::try_from(file).map_err(|error| error.to_string()))
                {
                    Ok(source) => {
                        if player.is_none() {
                            match DeviceSinkBuilder::open_default_sink() {
                                Ok(output) => {
                                    player = Some(Player::connect_new(output.mixer()));
                                    _stream = Some(output);
                                }
                                Err(error) => {
                                    let _ =
                                        events.send(AudioPlaybackEvent::Error(error.to_string()));
                                    continue;
                                }
                            }
                        }
                        duration = source
                            .total_duration()
                            .map_or(0.0, |value| value.as_secs_f64());
                        let player = player.as_ref().expect("output initialized");
                        player.append(source);
                        player.pause();
                        previous_position = Duration::MAX;
                        send_state(&events, 0.0, duration, false);
                    }
                    Err(error) => {
                        duration = 0.0;
                        let _ = events.send(AudioPlaybackEvent::Error(format!(
                            "Could not load audio file: {error}"
                        )));
                    }
                }
            }
            Ok(Command::SetPlaying(playing)) => {
                if let Some(player) = &player {
                    if playing {
                        player.play();
                        ended_sent = false;
                    } else {
                        player.pause();
                    }
                }
            }
            Ok(Command::Seek(seconds)) => {
                if let Some(player) = &player
                    && let Err(error) = player.try_seek(Duration::from_secs_f64(seconds))
                {
                    let _ = events.send(AudioPlaybackEvent::Error(error.to_string()));
                }
                previous_position = Duration::MAX;
                ended_sent = false;
            }
            Ok(Command::SetSpeed(speed)) => {
                if let Some(player) = &player {
                    player.set_speed(speed);
                }
            }
            Ok(Command::SetVolume(volume)) => {
                if let Some(player) = &player {
                    player.set_volume(volume);
                }
            }
            Ok(Command::Stop) => {
                if let Some(player) = &player {
                    player.stop();
                    player.clear();
                }
                duration = 0.0;
                previous_position = Duration::ZERO;
                was_playing = false;
                ended_sent = false;
                send_state(&events, 0.0, 0.0, false);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(player) = &player {
                    player.stop();
                }
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let Some(player) = &player else {
            continue;
        };
        let position = player.get_pos();
        let is_playing = !player.is_paused() && !player.empty();
        if position != previous_position || is_playing != was_playing {
            send_state(&events, position.as_secs_f64(), duration, is_playing);
            previous_position = position;
            was_playing = is_playing;
        }
        if duration > 0.0 && player.empty() && !ended_sent && previous_position != Duration::MAX {
            ended_sent = true;
            let _ = events.send(AudioPlaybackEvent::Ended);
        }
    }
}

fn send_state(events: &Sender<AudioPlaybackEvent>, position: f64, duration: f64, is_playing: bool) {
    let _ = events.send(AudioPlaybackEvent::State {
        position,
        duration,
        is_playing,
    });
}
