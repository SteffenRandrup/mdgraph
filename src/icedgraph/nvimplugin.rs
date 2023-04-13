use log;
use std::fmt;

use iced::futures;
use iced::subscription::{self, Subscription};

use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use neovim_lib::{Neovim, Session};


#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0
            .try_send(message)
            .expect("Send message to echo server");
    }
}


#[allow(clippy::large_enum_variant)]
enum State {
    Disconnected,
    Connected(NvimEventHandler, mpsc::Receiver<Message>)
}

#[derive(Debug, Clone)]
pub enum Message {
    Connected,
    Disconnected,
    User(String),
}

struct NvimEventHandler {
    nvim: Neovim,
    channel: std::sync::mpsc::Receiver<(std::string::String, Vec<neovim_lib::Value>)>
}

impl NvimEventHandler {

    fn new() -> NvimEventHandler {
        // TODO handle inability to get session
        let mut session = Session::new_parent().unwrap();
        let mut nvim = Neovim::new(session);
        let channel = nvim.session.start_event_loop_channel();
        NvimEventHandler { nvim, channel}
    }

}


#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    MessageReceived(Message),
}

pub fn connect() -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
        100,
        |mut output| async move {

            let mut state = State::Disconnected;

            loop {
                log::trace!("Loop start");
                match &mut state {
                    State::Disconnected => {
                        let (sender, receiver) = mpsc::channel(100);

                        log::trace!("Connecting");
                        state = State::Connected(NvimEventHandler::new(), receiver);

                        match output.send(Event::Connected(Connection(sender))).await {
                            Ok(_) => {
                                log::trace!("Connected!")
                            },
                            Err(_e) => {
                                log::trace!("In err branch")
                            }
                        };

                    },
                    State::Connected(nvim_handler, _input) => {

                        log::trace!("State connected");
                        std::thread::sleep(std::time::Duration::from_millis(15));
                        log::trace!("Waiting for message");
                        match nvim_handler.channel.recv() {
                            Ok(_message) => {
                                log::trace!("Got message");
                            },
                            Err(_e) => {
                                log::trace!("Error receiving");
                            }
                        };

                        // state = State::Disconnected;
                        // TODO send actual message or nothing
                        output.send(Event::MessageReceived(Message::Connected)).await.unwrap();

                    }
                }
            }
        }
    )

}

