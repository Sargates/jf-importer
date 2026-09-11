use jfi::catalog::*;
use jfi::media::ffprobe::*;

use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::oneshot::{channel, Sender, Receiver};
use futures::StreamExt;

use tracing::*;

pub enum ProbeResult {
    InProgress,
    Failure(FFprobeFailure),
    Success(FFprobeMediaInfo),
}

#[derive(Default)]
pub struct Prober {
    probe_results: HashMap<MediaItem, ProbeResult>,
    receivers: Vec<Receiver<(MediaItem, Result<FFprobeMediaInfo, FFprobeFailure>)>>,
}

impl Prober {
    pub fn dispatch(&mut self, item: MediaItem) {
        let (tx, rx) = channel::<(MediaItem, Result<FFprobeMediaInfo, FFprobeFailure>)>();

        let search_term = match &item {
            MediaItem::Movie(movie) => movie.search_term.clone(),
            MediaItem::Show(show) => show.search_term.clone(),
            MediaItem::Episode(ep) => /* ep.search_term.clone() */ todo!(),
        };
        let search_term_clone = search_term.clone();

        tracing::info!("Dispatching ffprobe for item: {:?}", search_term);

        self.probe_results.insert(item.clone(), ProbeResult::InProgress);

        // Dispatch the IO "globally" on the tokio runtime. This way we don't
        // need to deal with propagating `poll` methods up the widget tree
        //* tbh this is probably what I should have been doing already
        tokio::spawn(async move {
            let target = match &item {
                MediaItem::Movie(movie) => &movie.src,
                MediaItem::Show(show)   => &show.src,
                MediaItem::Episode(ep)  => &ep.src,
            };
            let payload = FFprobeMediaInfo::from_file(&target).await;

            tracing::info!("Probe completed for item: {:?}", search_term_clone);

            tx.send((item,payload));
        }.instrument(info_span!("Dispatch: ", search_term))
        );

        self.receivers.push(rx);
    }
    
    // maybe not the most efficient, but rather concise
    pub fn update(&mut self) {
        // single-threaded, replace is fine here
        let vec = core::mem::replace(&mut self.receivers, Vec::new());

        let (completed, incomplete): (Vec<_>, Vec<_>) = vec.into_iter()
            .partition(|rx| { !rx.is_empty() } );

        let _ = core::mem::replace(&mut self.receivers, incomplete);

        for mut item in completed {
            let (item, result) = item.try_recv().unwrap();
            let result = match result {
                Ok(i) => ProbeResult::Success(i),
                Err(e) => ProbeResult::Failure(e),
            };

            self.probe_results.insert(item, result);
        }
    }

    pub fn get(&self, item: &MediaItem) -> Option<&ProbeResult> {
        self.probe_results.get(item)
    }
}
