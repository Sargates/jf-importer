use std::ffi::OsString;
use std::sync::Arc;
use std::path::PathBuf;

use tokio::sync::*;

use crate::media::types::*;

// #[derive(Debug)]
// pub enum TreeGenError {
//     EnvVarNotSet,
//     EnvVarNotUnicode(OsString),
//     MoviesDirDoesntExist,
//     ShowsDirDoesntExist,
//     RegexError(regex::Error),
//     StatusReportSendError(watch::error::SendError<String>)
// }
// impl From<regex::Error> for TreeGenError {
//     fn from(value: regex::Error) -> Self {
//         Self::RegexError(value)
//     }
// }
// impl From<watch::error::SendError<String>> for TreeGenError {
//     fn from(value: watch::error::SendError<String>) -> Self {
//         Self::StatusReportSendError(value)
//     }
// }
//
// #[derive(Debug, Clone)]
// pub enum TreeNode {
//     Category {
//         name: String,
//         children: Vec<TreeNode>
//     },
//     Item {
//         inner: MediaItem,
//         children: Vec<TreeNode>
//     },
//     Fail {
//         err: MediaCreateError,
//         buf: PathBuf
//     },
// }
// impl Into<TreeNode> for Arc<Movie> {
//     fn into(self) -> TreeNode {
//         TreeNode::Item {
//             inner: MediaItem::Movie(self),
//             children: vec![]
//         }
//     }
// }
// impl Into<TreeNode> for Arc<Show> {
//     fn into(self) -> TreeNode {
//         let new_children = self.episodes.try_lock().unwrap().iter().map(|ep| ep.clone().into()).collect();
//         TreeNode::Item {
//             inner: MediaItem::Show(self),
//             children: new_children,
//         }
//     }
// }
// impl Into<TreeNode> for Arc<Episode> {
//     fn into(self) -> TreeNode {
//         TreeNode::Item {
//             inner: MediaItem::Episode(self),
//             children: vec![]
//         }
//     }
// }
// impl TreeNode {
//     pub fn children(&self) -> Option<&Vec<TreeNode>> {
//         match self {
//             TreeNode::Category{ name, children } => Some(children),
//             TreeNode::Item{ inner, children }    => Some(children),
//             _                                    => None
//         }
//     }
//     pub fn children_mut(&mut self) -> Option<&mut Vec<TreeNode>> {
//         match self {
//             TreeNode::Category{ name, children } => Some(children),
//             TreeNode::Item{ inner, children }    => Some(children),
//             _                                    => None
//         }
//         }
//     pub fn push_child(&mut self, child: TreeNode) {
//         match self {
//             TreeNode::Category{ name, children } => { children.push(child); }
//             TreeNode::Item{ inner, children }    => { children.push(child); }
//             _                                    => panic!("Expected variant with children")
//         }
//     }
//     // pub fn is_queried(&self) -> bool {
//     //     match self {
//     //         TreeNode::Item { inner, children } => {
//     //             match *API_CALLS.get_query(inner).unwrap() {
//     //                 QueryStatus::NotStarted => false,
//     //                 _                       => false
//     //             }
//     //         }
//     //         _ => panic!("Expected Movie, Show, or Episode!")
//     //     }
//     // }
// }
//
// //* This is based on the old global implementation. I haven't bothered figuring out how to couple an API response to a TreeNode yet.
// // impl std::fmt::Display for TreeNode {
// //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// //         // writeln!(f, "Printing: {:?}", self);
// //         match self {
// //             TreeNode::Category{ name, children } => {
// //                 write!(f, "{name}")
// //             }
// //             TreeNode::Item { inner, children } => {
// //                 match &inner {
// //                     MediaItem::Show(show) => {
// //                         // let r_status = API_CALLS.get_query(inner).unwrap();
// //                         match &*r_status {
// //                             QueryStatus::Success(response) => {
// //                                 write!(f, "{} ({}) ", response.title, response.year);
// //                                 if let Some(imdb) = &response.imdb {
// //                                     write!(f, "[imdbid-{}]", imdb) } 
// //                                 else { write!(f, "[tmdbid-{}]", response.tmdb) }
// //                             }
// //                             _ => write!(f, "[{r_status:?}] {}", show.src.to_string_lossy()),
// //                         }
// //                     }
// //                     MediaItem::Movie(movie) => {
// //                         let r_status = API_CALLS.get_query(inner).unwrap();
// //                         match &*r_status {
// //                             QueryStatus::Success(response) => {
// //                                 write!(f, "{} ({}) ", response.title, response.year);
// //                                 if let Some(imdb) = &response.imdb {
// //                                     write!(f, "[imdbid-{}]", imdb) } 
// //                                 else { write!(f, "[tmdbid-{}]", response.tmdb) }
// //                             }
// //                             _ => write!(f, "[{r_status:?}] {}", movie.src.to_string_lossy()),
// //                         }
// //                     }
// //                     MediaItem::Episode(ep) => {
// //                         // I used to get a deadlock if I used `block_on` directly, but doing it this way is better
// //                         // because there shouldn't be any risk of deadlocking in synchronous code.
// //                         let parent_item = MediaItem::Show(Weak::upgrade(&ep.parent).unwrap());
// //                         match (&*API_CALLS.get_query(&inner).unwrap(), &*API_CALLS.get_query(&parent_item).unwrap()) {
// //
// //                             (_, QueryStatus::Success(response)) => {
// //                                 write!(f, "{} ({}) ", response.title, response.year);
// //                                 if let Some(imdb) = &response.imdb {
// //                                     write!(f, "[imdbid-{}]", imdb) } 
// //                                 else { write!(f, "[tmdbid-{}]", response.tmdb) }
// //
// //                                 // match &*parent_guard {
// //                                 //     QueryStatus::Success(response) => write!(f, "{} {}", response.title.clone(), ep.id),
// //                                 //     QueryStatus::Failed(err)       => write!(f, "[Parent Failure]: {}", ep.src.to_string_lossy()),
// //                                 //     QueryStatus::NotStarted        => write!(f, "[Parent Empty]: {}", ep.src.to_string_lossy()),
// //                                 //     QueryStatus::InProgress        => write!(f, "Parent query is in progress: {}", ep.src.to_string_lossy()),
// //                                 // }
// //                             },
// //                             (_, _)
// //                                 => write!(f, "Parent query Failed: {}", ep.src.to_string_lossy()),
// //                             _ => unreachable!()
// //                         }
// //
// //                     }
// //                 }
// //             }
// //             TreeNode::Fail{ err, buf } => {
// //                 write!(f, "Parse Failure: {buf:?}")
// //             }
// //         }
// //     }
// // }
