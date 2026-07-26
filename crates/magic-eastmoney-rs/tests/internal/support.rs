use crate::{EastmoneyError, EastmoneyTransport};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

type ScriptedResponse = Result<Vec<u8>, EastmoneyError>;
type SharedResponses = Arc<Mutex<VecDeque<ScriptedResponse>>>;

#[derive(Clone)]
pub(crate) struct ScriptedTransport {
    responses: SharedResponses,
    requests: Arc<Mutex<Vec<String>>>,
}

impl ScriptedTransport {
    pub(crate) fn from_bodies(bodies: impl IntoIterator<Item = &'static [u8]>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                bodies.into_iter().map(|body| Ok(body.to_vec())).collect(),
            )),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn from_results(responses: impl IntoIterator<Item = ScriptedResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.requests)
    }

    fn respond(&self, request: String) -> Result<Vec<u8>, EastmoneyError> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(EastmoneyError::Transport(
                    "scripted transport has no remaining response".into(),
                ))
            })
    }
}

impl EastmoneyTransport for ScriptedTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.respond(format!("GET {url}"))
    }

    fn post_json(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.respond(format!("POST {url} {}", String::from_utf8_lossy(body)))
    }
}
