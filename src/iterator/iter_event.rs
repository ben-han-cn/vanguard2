use super::delegation_point::DelegationPoint;
use super::host_selector::Host;
use crate::types::Query;
use r53::{Message, RRset};

#[derive(Copy, Clone)]
pub enum QueryState {
    InitQuery,
    QueryTarget,
    QueryResponse,
    PrimeResponse,
    TargetResponse,
    Finished,
}

#[derive(Copy, Clone)]
pub enum ResponseType {
    Answer,
    CName,
    Throwaway,
    Lame,
}

pub struct IterEvent {
    base_event: Option<Box<IterEvent>>,

    request: Message,
    response: Option<Message>,
    response_type: Option<ResponseType>,

    state: QueryState,
    final_state: QueryState,
    prepend_rrsets: Vec<RRset>,
    delegation_point: Option<DelegationPoint>,

    pub target_queries: u8,
    pub current_quries: u8,
    pub query_restart_count: u8,
    pub referral_count: u8,
}

impl IterEvent {
    pub fn new(request: Message, init_state: QueryState, final_state: QueryState) -> Self {
        Self {
            base_event: None,
            request: request,
            response: None,
            response_type: None,
            state: init_state,
            final_state,
            prepend_rrsets: Vec::new(),
            delegation_point: None,
            target_queries: 0,
            current_quries: 0,
            query_restart_count: 0,
            referral_count: 0,
        }
    }

    pub fn set_delegation_point(&mut self, delegation_point: DelegationPoint) {
        self.delegation_point = Some(delegation_point);
    }

    pub fn get_state(&self) -> QueryState {
        self.state
    }

    pub fn get_final_state(&self) -> QueryState {
        self.final_state
    }

    pub fn set_state(&mut self, state: QueryState) {
        self.state = state;
    }

    pub fn get_depth(&self) -> u8 {
        if let Some(ref base_event) = self.base_event {
            base_event.get_depth() + 1
        } else {
            0
        }
    }

    pub fn get_request(&self) -> &Message {
        &self.request
    }

    pub fn set_response(&mut self, response: Message, typ: ResponseType) {
        self.response = Some(response);
        self.response_type = Some(typ);
    }

    pub fn get_response(&mut self) -> anyhow::Result<Message> {
        let response = self.response.take().unwrap();
        Ok(response)
    }
}
