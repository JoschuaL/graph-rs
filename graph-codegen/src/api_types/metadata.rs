use crate::api_types::RequestTask;
use crate::parser::HttpMethod;
use std::collections::BTreeMap;

pub trait Metadata {
    /// A description of what the request is doing.
    fn doc(&self) -> Option<String>;

    /// The HTTP method for the request. Must be one of GET, PUT, POST, PATCH, DELETE
    /// Macro type: expr
    fn http_method(&self) -> HttpMethod;

    /// The method name that is used to call this request.
    /// Macro type: ident
    fn fn_name(&self) -> String;

    /// Key value pair of url queries to include as method parameters.
    /// The key in the BTreeMap is the actual key of the query in the url. The value
    /// is the name of the method parameter.
    /// For example: [ key: "$deltaToken", value: delta_token ]
    fn queries(&self) -> BTreeMap<String, String>;

    /// The request task describes the type of action this request will perform.
    fn request_task(&self) -> RequestTask;

    /// Does the request require a body.
    fn has_body(&self) -> bool;

    /// The struct that the method is implemented for.
    fn parent(&self) -> String;

    /// The macro call name such as `vec!`
    fn macro_fn_name(&self) -> String {
        let http_method = self.http_method();
        match self.request_task() {
            RequestTask::NoContent
            | RequestTask::Json
            | RequestTask::Bytes
            | RequestTask::Upload
            | RequestTask::UploadSession
            | RequestTask::Delta => http_method.to_string(),
            RequestTask::Download => "download".to_string(),
            RequestTask::AsyncDownload => "async_download".to_string(),
        }
    }
}
