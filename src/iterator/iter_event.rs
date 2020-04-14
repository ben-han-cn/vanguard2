use std::time::Instant;

use super::delegation_point::DelegationPoint;
use super::message_helper::ResponseCategory;
use crate::types::Response;
use r53::{
    message::Section, message::SectionType, HeaderFlag, Message, MessageBuilder, RRset, Rcode,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QueryState {
    InitQuery,
    QueryTarget,
    QueryResponse,
    PrimeResponse,
    TargetResponse,
    Finished,
}

pub struct IterEvent {
    pub start_time: Instant,
    base_event: Option<Box<IterEvent>>,

    orignal_request: Message,
    current_request: Option<Message>,

    response: Option<Message>,
    response_category: Option<ResponseCategory>,

    state: QueryState,
    final_state: QueryState,
    prepend_rrsets: Vec<RRset>,
    delegation_point: Option<DelegationPoint>,

    pub cache_hit: bool,
    pub error_count: u8,
    pub query_restart_count: u8,
    pub referral_count: u8,
}

impl IterEvent {
    pub fn new(request: Message, init_state: QueryState, final_state: QueryState) -> Self {
        Self {
            start_time: Instant::now(),
            base_event: None,
            orignal_request: request,
            current_request: None,
            response: None,
            response_category: None,
            state: init_state,
            final_state,
            prepend_rrsets: Vec::new(),
            delegation_point: None,
            cache_hit: false,
            error_count: 0,
            query_restart_count: 0,
            referral_count: 0,
        }
    }

    pub fn set_delegation_point(&mut self, delegation_point: DelegationPoint) {
        self.delegation_point = Some(delegation_point);
    }

    pub fn get_delegation_point(&self) -> Option<&DelegationPoint> {
        self.delegation_point.as_ref()
    }

    pub fn get_mut_delegation_point(&mut self) -> Option<&mut DelegationPoint> {
        self.delegation_point.as_mut()
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

    pub fn add_prepend_rrsets(&mut self, mut rrsets: Vec<RRset>) {
        self.prepend_rrsets.append(&mut rrsets)
    }

    pub fn set_response(&mut self, response: Message, category: ResponseCategory) {
        self.response = Some(response);
        self.response_category = Some(category);
    }

    pub fn take_response(&mut self) -> (Message, ResponseCategory) {
        (
            self.response.take().unwrap(),
            self.response_category.take().unwrap(),
        )
    }

    pub fn set_base_event(&mut self, e: IterEvent) {
        assert!(self.base_event.is_none());
        self.base_event = Some(Box::new(e));
    }

    pub fn take_base_event(&mut self) -> Option<IterEvent> {
        self.base_event.take().map(|e| *e)
    }

    pub fn generate_final_response(mut self) -> Response {
        let mut response = self.response.take().expect("should has response");
        if response.header.rcode != Rcode::ServFail && !self.prepend_rrsets.is_empty() {
            if let Some(answers) = response.section_mut(SectionType::Answer) {
                self.prepend_rrsets.append(answers);
            }
            response.sections[0] = Section(Some(self.prepend_rrsets));
        }
        response.question = self.orignal_request.question.take();
        response.edns = self.orignal_request.edns.take();

        let mut builder = MessageBuilder::new(&mut response);
        builder.set_flag(HeaderFlag::RecursionAvailable);
        builder.clear_flag(HeaderFlag::AuthAnswer);
        builder.id(self.orignal_request.header.id);
        builder.done();
        let mut resp = Response::new(response);
        resp.cache_hit = self.cache_hit;
        resp
    }
}
