use super::delegation_point::DelegationPoint;
use super::host_selector::Host;
use crate::types::Query;
use r53::{Message, RRset};

#[derive(Copy, Clone, PartialEq, Eq)]
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
    Unknown,
    Answer,
    CName,
    Throwaway,
    Lame,
}

pub struct IterEvent {
    base_event: Option<Box<IterEvent>>,

    orignal_request: Message,
    current_request: Option<Message>,

    response: Option<Message>,
    response_type: Option<ResponseType>,

    state: QueryState,
    final_state: QueryState,
    prepend_rrsets: Vec<RRset>,
    delegation_point: Option<DelegationPoint>,

    pub target_queries: u8,
    pub current_queries: u8,
    pub query_restart_count: u8,
    pub referral_count: u8,
}

impl IterEvent {
    pub fn new(request: Message, init_state: QueryState, final_state: QueryState) -> Self {
        Self {
            base_event: None,
            orignal_request: request,
            current_request: None,
            response: None,
            response_type: None,
            state: init_state,
            final_state,
            prepend_rrsets: Vec::new(),
            delegation_point: None,
            target_queries: 0,
            current_queries: 0,
            query_restart_count: 0,
            referral_count: 0,
        }
    }

    pub fn set_delegation_point(&mut self, delegation_point: DelegationPoint) {
        self.delegation_point = Some(delegation_point);
    }

    pub fn take_delegation_point(&mut self) -> Option<DelegationPoint> {
        self.delegation_point.take()
    }

    pub fn get_state(&self) -> QueryState {
        self.state
    }

    pub fn get_final_state(&self) -> QueryState {
        self.final_state
    }

    pub fn next_state(&mut self, state: QueryState) {
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
        if let Some(ref request) = self.current_request {
            return request;
        }
        &self.orignal_request
    }

    pub fn set_current_request(&mut self, request: Message) {
        self.current_request = Some(request);
    }

    pub fn get_original_request(&self) -> &Message {
        &self.orignal_request
    }

    pub fn set_prepend_rrsets(&mut self, rrsets: Vec<RRset>) {
        self.prepend_rrsets = rrsets
    }

    pub fn set_response(&mut self, response: Message, typ: ResponseType) {
        self.response = Some(response);
        self.response_type = Some(typ);
    }

    pub fn get_response(&mut self) -> Option<Message> {
        self.response.take()
    }

    pub fn set_base_event(&mut self, e: IterEvent) {
        assert!(self.base_event.is_none());
        self.base_event = Some(Box::new(e));
    }
}
