use dashchat_node::Node;

use crate::node::node_context::NodeContext;

/// A Node that is owned by this process, together with the context it was
/// built for.
#[derive(Clone)]
pub struct AppNode {
    /// The context that describes this Node's role and channels.
    pub context: NodeContext,
    /// The Node itself.
    pub node: Node,
}

impl AppNode {
    /// Create a new `AppNode` from the given context and Node.
    pub fn new(context: NodeContext, node: Node) -> Self {
        Self { context, node }
    }

    /// Whether this Node can be reused to satisfy a request for the given
    /// context.
    pub fn is_compatible_with(&self, context: &NodeContext) -> bool {
        self.context.is_compatible_with(context)
    }
}
