use super::protocol::{classify, ClassifiedFrame, Event};

#[test]
fn classifies_state_changed_event() {
    let text = r#"{"event":"state_changed","data":{"pipeline_id":"0","old_state":"paused","new_state":"playing"}}"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::StateChanged {
            pipeline_id,
            new_state,
            ..
        }) => {
            assert_eq!(pipeline_id, "0");
            assert_eq!(new_state, "playing");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn classifies_response_with_string_id() {
    let text = r#"{"id":"abc","result":{"pipeline_id":"0"}}"#;
    match classify(text) {
        ClassifiedFrame::Response(response) => {
            assert_eq!(response.id_as_str(), Some("abc".to_owned()));
            assert!(response.error.is_none());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn unknown_event_falls_back_to_other() {
    let text = r#"{"event":"future_unknown","data":{}}"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::Other) => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn deserializes_a_full_pipeline_added_event() {
    let text = r#"{"event":"pipeline_added","data":{"pipeline_id":"7","description":"videotestsrc ! autovideosink"}}"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::PipelineAdded {
            pipeline_id,
            description,
        }) => {
            assert_eq!(pipeline_id, "7");
            assert_eq!(description, "videotestsrc ! autovideosink");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn response_with_error_does_not_yield_result() {
    let text = r#"{"id":"abc","error":{"code":-32000,"message":"Pipeline not found"}}"#;
    match classify(text) {
        ClassifiedFrame::Response(response) => {
            assert!(response.result.is_none());
            assert_eq!(response.error.as_ref().unwrap().code, -32000);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn integer_id_responses_round_trip_to_string() {
    let text = r#"{"id":42,"result":{}}"#;
    match classify(text) {
        ClassifiedFrame::Response(response) => {
            assert_eq!(response.id_as_str(), Some("42".to_owned()));
        }
        other => panic!("unexpected: {other:?}"),
    }
}
