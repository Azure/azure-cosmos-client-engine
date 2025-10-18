// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{fmt::Debug, str::FromStr};

use crate::ErrorKind;

use super::{producer::ItemProducer, QueryResult};

#[derive(Debug)]
pub struct PipelineNodeResult {
    /// The produced result, if any.
    ///
    /// If the node returns no result, it does NOT guarantee that the pipeline has terminated.
    /// It only means that more data has to be provided to the pipeline before a result can be produced.
    pub value: Option<QueryResult>,

    /// A boolean indicating if the pipeline should terminate after this result.
    ///
    /// If set, the pipeline should be terminated after yielding the item in [`PipelineNodeResult::value`], if any.
    pub terminated: bool,
}

impl PipelineNodeResult {
    /// Indicates that the pipeline should terminate after yielding the item in [`PipelineNodeResult::value`], if any.
    pub const EARLY_TERMINATE: Self = Self {
        value: None,
        terminated: true,
    };

    /// Indicates that the pipeline has no result, but is not terminated. The pipeline requires more data to produce a result.
    pub const NO_RESULT: Self = Self {
        value: None,
        terminated: false,
    };

    pub fn result(value: QueryResult, terminated: bool) -> Self {
        Self {
            value: Some(value),
            terminated,
        }
    }
}

/// Represents a slice of the query pipeline.
///
/// The pipeline is made up of all the nodes in the pipeline, with the final node being the item producer.
/// This struct represents some subset of the nodes, and the item producer.
///
/// This type exists so that nodes don't have to deal with slicing the list of nodes, and so that the item producer can be passed around easily.
/// Since the Item Producer and Pipeline Nodes are both owned by the [`QueryPipeline`](super::QueryPipeline), we can't create an owned type that contains both.
pub struct PipelineSlice<'a> {
    nodes: &'a mut [Box<dyn PipelineNode>],
    producer: &'a mut ItemProducer,
}

impl<'a> PipelineSlice<'a> {
    pub fn new(nodes: &'a mut [Box<dyn PipelineNode>], producer: &'a mut ItemProducer) -> Self {
        Self { nodes, producer }
    }

    /// Retrieves the next item from the first node in the span, passing the rest of the span as the "next" parameter.
    pub fn run(&mut self) -> crate::Result<PipelineNodeResult> {
        match self.nodes.split_first_mut() {
            Some((node, rest)) => {
                let result = node.next_item(PipelineSlice {
                    nodes: rest,
                    producer: self.producer,
                });
                tracing::debug!(node_name = ?node.name(), ?result, "completed pipeline node");
                result
            }
            None => {
                tracing::debug!("retrieving item from producer");
                let value = self.producer.produce_item()?;
                Ok(PipelineNodeResult {
                    value,
                    terminated: false,
                })
            }
        }
    }
}

/// Represents a node in the query pipeline.
///
/// Nodes are the building blocks of the query pipeline. They are used to represent different stages of query execution, such as filtering, sorting, and aggregation.
pub trait PipelineNode: Send {
    /// Retrieves the next item from this node in the pipeline.
    ///
    /// # Parameters
    /// * `next` - The next node in the pipeline, or `Ok(None)` if this is the last node in the pipeline.
    fn next_item(&mut self, rest: PipelineSlice) -> crate::Result<PipelineNodeResult>;

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

impl PipelineNode for LimitPipelineNode {
    fn next_item(&mut self, mut rest: PipelineSlice) -> crate::Result<PipelineNodeResult> {
        if self.remaining == 0 {
            tracing::debug!("limit reached, terminating pipeline");
            return Ok(PipelineNodeResult::EARLY_TERMINATE);
        }

        match rest.run()? {
            PipelineNodeResult {
                value: Some(item),
                terminated,
            } => {
                tracing::debug!("limit not yet reached, returning item");
                self.remaining -= 1;
                Ok(PipelineNodeResult::result(
                    item,
                    terminated || self.remaining == 0,
                ))
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

impl PipelineNode for OffsetPipelineNode {
    fn next_item(&mut self, mut rest: PipelineSlice) -> crate::Result<PipelineNodeResult> {
        while self.remaining > 0 {
            match rest.run()? {
                PipelineNodeResult { value: Some(_), .. } => {
                    tracing::debug!("offset not reached, skipping item");
                    self.remaining -= 1
                }

                // Pass through any early terminations or no results.
                x => return Ok(x),
            }
        }

        // Now, we're no longer skipping items, so we can pass through the rest of the pipeline.
        tracing::debug!("offset reached, returning item");
        rest.run()
    }
}

#[derive(Clone, Debug)]
pub enum Aggregator {
    Count,
    Sum,
    Average,
    Min,
    Max,
}

impl FromStr for Aggregator {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        // A match statement seems like the right thing to do, but it means forcing the string to lowercase first.
        // This allows us to do the comparison in a case-insensitive way without having to allocate a new string.
        if s.eq_ignore_ascii_case("count") {
            Ok(Aggregator::Count)
        } else if s.eq_ignore_ascii_case("sum") {
            Ok(Aggregator::Sum)
        } else if s.eq_ignore_ascii_case("average") {
            Ok(Aggregator::Average)
        } else if s.eq_ignore_ascii_case("min") {
            Ok(Aggregator::Min)
        } else if s.eq_ignore_ascii_case("max") {
            Ok(Aggregator::Max)
        } else {
            Err(ErrorKind::UnsupportedQueryPlan.with_message(format!("unknown aggregator: {}", s)))
        }
    }
}

impl Aggregator {
    /// Aggregates the current value with the provided value, updating it in place.
    pub fn aggregate(self, current: &mut serde_json::Value) -> crate::Result<()> {
        todo!()
    }
}

pub struct AggregatePipelineNode {
    aggregators: Vec<Aggregator>,
    values: Vec<serde_json::Value>,
}

impl AggregatePipelineNode {
    pub fn from_names(names: Vec<String>) -> crate::Result<Self> {
        let mut aggregators = Vec::with_capacity(names.len());
        for name in names {
            aggregators.push(Aggregator::from_str(&name)?);
        }
        let values = vec![serde_json::Value::Null; aggregators.len()];
        Ok(Self {
            aggregators,
            values,
        })
    }
}

impl PipelineNode for AggregatePipelineNode {
    fn next_item(&mut self, mut rest: PipelineSlice) -> crate::Result<PipelineNodeResult> {
        todo!()
    }
}
