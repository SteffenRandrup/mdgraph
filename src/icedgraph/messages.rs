use fdg_sim::petgraph::graph::NodeIndex;

// Enum to send messages in iced program
#[derive(Debug, Clone)]
pub enum GMessage {
    GraphClick(Option<NodeIndex>),
    GraphicsTick,
}
