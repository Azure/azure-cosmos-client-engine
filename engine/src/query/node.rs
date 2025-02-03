use super::{producer::ItemProducer, QueryResult};

pub enum PipelineResult {
    /// Indicates that a query result was produced.
    Result(QueryResult),

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
/// Since the Item Producer and Pipeline Nodes are both owned by the [`QueryPipeline`], we can't create an owned type that contains both.
pub struct PipelineSlice<'a> {
    nodes: &'a mut [Box<dyn PipelineNode>],
    producer: &'a mut ItemProducer,
}

impl<'a> PipelineSlice<'a> {
    pub fn new(nodes: &'a mut [Box<dyn PipelineNode>], producer: &'a mut ItemProducer) -> Self {
        Self { nodes, producer }
    }

    /// Retrieves the next item from the first node in the span, passing the rest of the span as the "next" parameter.
    pub fn next_item(&mut self) -> crate::Result<PipelineResult> {
        match self.nodes.split_first_mut() {
            Some((node, rest)) => node.next_item(PipelineSlice {
                nodes: rest,
                producer: self.producer,
            }),
            None => match self.producer.produce_item()? {
                Some(item) => Ok(PipelineResult::Result(item)),
                None => Ok(PipelineResult::NoResult),
            },
        }
    }
}

/// Represents a node in the query pipeline.
///
/// Nodes are the building blocks of the query pipeline. They are used to represent different stages of query execution, such as filtering, sorting, and aggregation.
pub trait PipelineNode {
    /// Retrieves the next item from this node in the pipeline.
    ///
    /// # Parameters
    /// * `next` - The next node in the pipeline, or `Ok(None)` if this is the last node in the pipeline.
    fn next_item(&mut self, rest: PipelineSlice) -> crate::Result<PipelineResult>;
}

/// A pipeline node that limits the number of items that can pass through it by a fixed number.
///
/// This can be used to implement both `TOP` and `LIMIT` clauses in a query.
pub struct LimitPipelineNode {
    remaining: u64,
}

impl LimitPipelineNode {
    pub fn new(limit: u64) -> Self {
        Self { remaining: limit }
    }
}

impl PipelineNode for LimitPipelineNode {
    fn next_item(&mut self, mut rest: PipelineSlice) -> crate::Result<PipelineResult> {
        if self.remaining == 0 {
            // There's no need to continue executing the pipeline. The limit has been reached.
            return Ok(PipelineResult::EarlyTermination);
        }

        self.remaining -= 1;

        rest.next_item()
    }
}
