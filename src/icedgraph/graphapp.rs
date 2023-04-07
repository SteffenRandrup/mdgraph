use iced::executor;
use iced::theme::Palette;
use iced::widget::Canvas;
use iced::{Application, Command, Element, Subscription};
use iced::{Color, Length, Theme};

use fdg_sim::{self, force::handy, petgraph::Undirected};
use fdg_sim::{ForceGraph, Simulation, SimulationParameters};

use crate::icedgraph::graphcanvas::{GraphDisplay, GraphState};
use crate::icedgraph::messages::GMessage;

pub struct GraphApp<N, E> {
    simulation: Simulation<N, E, Undirected>,
    update_step_counter: usize,
    simulation_step_size: f32,
    graph_state: GraphState,
}

pub struct GraphAppFlags<N, E> {
    graph: ForceGraph<N, E>,
}

impl<N, E> GraphAppFlags<N, E> {
    pub fn from_graph(graph: ForceGraph<N, E>) -> GraphAppFlags<N, E> {
        return GraphAppFlags { graph };
    }
}

impl<N, E> Application for GraphApp<N, E> {
    type Executor = executor::Default;
    type Flags = GraphAppFlags<N, E>;
    type Message = GMessage;
    type Theme = Theme;

    fn new(flags: GraphAppFlags<N, E>) -> (Self, Command<Self::Message>) {
        let graph = flags.graph;

        let simforce = handy(200.0, 0.9, true, true);
        let params = SimulationParameters::new(200.0, fdg_sim::Dimensions::Two, simforce);

        return (
            Self {
                simulation: Simulation::from_graph(graph, params),
                update_step_counter: 0,
                simulation_step_size: 0.055,
                graph_state: GraphState::default(),
            },
            Command::none(),
        );
    }

    fn title(&self) -> String {
        String::from("Markdown Links")
    }

    fn theme(&self) -> Theme {
        Theme::custom(Palette {
            background: Color::from_rgba8(46, 52, 64, 1.0),
            text: Color::from_rgba8(229, 233, 240, 1.0),
            primary: Color::from_rgba8(216, 222, 233, 0.3),
            success: Color::from_rgba8(136, 192, 208, 1.0),
            danger: Color::from_rgba8(191, 97, 106, 1.0),
        })
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            // Event from Canvas
            GMessage::GraphClick(node_index_option) => {
                match node_index_option {
                    Some(node_index) => {
                        let node = &self.simulation.get_graph()[node_index];
                        self.graph_state.active_node = Some(node_index);
                        println!("Clicked: {:?}", node.name);
                    }
                    None => {
                        println!("Not clicked on a node")
                    }
                }

                // We are allowed to update the graph display again
                self.update_step_counter = 0;
            }

            // Timebased tick to update simulation
            // Don't update the simulation forever
            // Save resources
            GMessage::GraphicsTick => {
                if self.update_step_counter < 1000 {
                    self.simulation.update(self.simulation_step_size);
                    self.update_step_counter += 1;
                }
                self.graph_state.visualization_frac =
                    (self.graph_state.visualization_frac + 1.0 / 120.0) % 1.0;
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        return Canvas::new(GraphDisplay::new(
            self.simulation.get_graph(),
            &self.graph_state,
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    // Continuously update the graph (15ms ~ 60fps)
    // Might not want to set a fixed time
    fn subscription(&self) -> Subscription<Self::Message> {
        iced::time::every(std::time::Duration::from_millis(15)).map(|_| GMessage::GraphicsTick)
    }
}
