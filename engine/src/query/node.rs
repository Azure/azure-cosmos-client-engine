use std::fmt::Debug;

use super::{producer::ItemProducer, QueryClauseItem, QueryResult};

pub enum PipelineNodeResult<T: Debug, I: QueryClauseItem> {
    /// Indicates that a query result was produced.
    Result(QueryResult<T, I>),

    /// Indicates that no result was produced, but the pipeline may still produce results if additional data is provided.
    NoResult,

    /// Indicates that a node in the pipeline terminated the entire pipeline early.
    ///
    /// As an example, the [`LimitPipelineNode`] will return this result when it has reached its limit, as it is impossible to continue executing the pipeline once the limit is reached.
    EarlyTermination,
}

/// Represents a slice of the query pipeline.
///
/// The pipeline is made up of all the nodes in the pipeline, with the final node being the item producer.
/// This struct represents some subset of the nodes, and the item producer.
///
/// This type exists so that nodes don't have to deal with slicing the list of nodes, and so that the item producer can be passed around easily.
/// Since the Item Producer and Pipeline Nodes are both owned by the [`QueryPipeline`](super::QueryPipeline), we can't create an owned type that contains both.
pub struct PipelineSlice<'a, T: Debug, I: QueryClauseItem> {
    nodes: &'a mut [Box<dyn PipelineNode<T, I>>],
    producer: &'a mut ItemProducer<T, I>,
}

impl<'a, T: Debug, I: QueryClauseItem> PipelineSlice<'a, T, I> {
    pub fn new(
        nodes: &'a mut [Box<dyn PipelineNode<T, I>>],
        producer: &'a mut ItemProducer<T, I>,
    ) -> Self {
        Self { nodes, producer }
    }

    /// Retrieves the next item from the first node in the span, passing the rest of the span as the "next" parameter.
    pub fn next_item(&mut self) -> crate::Result<PipelineNodeResult<T, I>> {
        match self.nodes.split_first_mut() {
            Some((node, rest)) => {
                tracing::trace!(node_name = ?node.name(), "running pipeline node");
                node.next_item(PipelineSlice {
                    nodes: rest,
                    producer: self.producer,
                })
            }
            None => {
                tracing::trace!("retrieving item from producer");
                match self.producer.produce_item()? {
                    Some(item) => Ok(PipelineNodeResult::Result(item)),
                    None => Ok(PipelineNodeResult::NoResult),
                }
            }
        }
    }
}

/// Represents a node in the query pipeline.
///
/// Nodes are the building blocks of the query pipeline. They are used to represent different stages of query execution, such as filtering, sorting, and aggregation.
pub trait PipelineNode<T: Debug, I: QueryClauseItem>: Send {
    /// Retrieves the next item from this node in the pipeline.
    ///
    /// # Parameters
    /// * `next` - The next node in the pipeline, or `Ok(None)` if this is the last node in the pipeline.
    fn next_item(&mut self, rest: PipelineSlice<T, I>) -> crate::Result<PipelineNodeResult<T, I>>;

    /// Retrieves the name of this node, which defaults to it's type name.
    fn name(&self) -> &'static str {
        std::any::type_name_of_val(self)
    }
}

/// A pipeline node that limits the number of items that can pass through it by a fixed number.
///
/// This can be used to implement both `TOP` and `LIMIT` clauses in a query.
pub struct LimitPipelineNode {
    /// The number of items that can pass through this node before it terminates the pipeline.
    remaining: u64,
}

impl LimitPipelineNode {
    pub fn new(limit: u64) -> Self {
        Self { remaining: limit }
    }
}

impl<T: Debug, I: QueryClauseItem> PipelineNode<T, I> for LimitPipelineNode {
    fn next_item(
        &mut self,
        mut rest: PipelineSlice<T, I>,
    ) -> crate::Result<PipelineNodeResult<T, I>> {
        if self.remaining == 0 {
            tracing::trace!("limit reached, terminating pipeline");
            return Ok(PipelineNodeResult::EarlyTermination);
        }

        match rest.next_item()? {
            PipelineNodeResult::Result(item) => {
                tracing::trace!("limit not yet reached, returning item");
                self.remaining -= 1;
                Ok(PipelineNodeResult::Result(item))
            }

            // Pass through other results
            x => Ok(x),
        }
    }
}

/// A pipeline node that skips a fixed number of items before allowing any items to pass through it.
///
/// This can be used to implement both `OFFSET` clauses in a query.
pub struct OffsetPipelineNode {
    /// The number of items that should be skipped before allowing any items to pass through.
    remaining: u64,
}

impl OffsetPipelineNode {
    pub fn new(offset: u64) -> Self {
        Self { remaining: offset }
    }
}

impl<T: Debug, I: QueryClauseItem> PipelineNode<T, I> for OffsetPipelineNode {
    fn next_item(
        &mut self,
        mut rest: PipelineSlice<T, I>,
    ) -> crate::Result<PipelineNodeResult<T, I>> {
        while self.remaining > 0 {
            match rest.next_item()? {
                PipelineNodeResult::Result(_) => {
                    tracing::trace!("offset not reached, skipping item");
                    self.remaining -= 1
                }

                // Pass through any early terminations or no results.
                x => return Ok(x),
            }
        }

        // Now, we're no longer skipping items, so we can pass through the rest of the pipeline.
        tracing::trace!("offset reached, returning item");
        rest.next_item()
    }
}
