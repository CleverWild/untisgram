use std::net::IpAddr;

use crate::error;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Error codes contained in [Untis API errors](Error).
/// The underlying integer can be accessed using [code.as_isize()](Self::as_isize()).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ErrorCode {
    UserBlocked = -8998,
    NotAuthenticated = -8520,
    NoAccess = -8509,
    InvalidCredentials = -8504,
    InvalidSchoolName = -8500,
    TooManyResults = -6003,
}

impl ErrorCode {
    pub fn as_isize(&self) -> isize {
        *self as isize
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct Request<'a, P: Serialize> {
    jsonrpc: &'static str,
    id: &'a str,
    method: &'a str,
    params: P,
}

impl<'a, P: Serialize> Request<'a, P> {
    pub fn new(id: &'a str, method: &'static str, params: P) -> Self {
        Self {
            id,
            method,
            jsonrpc: "2.0",
            params,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Response<T> {
    Ok {
        jsonrpc: String,
        id: String,
        result: T,
    },
    Err {
        jsonrpc: String,
        id: String,
        error: Error,
    },
}

/// JSON-RPC error object.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Error {
    pub code: isize,
    pub message: String,
}

pub struct Client {
    pub http_client: reqwest::Client,
    url: String,
    last_request_id: usize,
}

impl Client {
    pub fn new(url: &str) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .hickory_dns(true)
            .build()
            .unwrap();
        Self {
            http_client: client,
            url: url.to_string(),
            last_request_id: 0,
        }
    }

    pub fn new_with_bind_address(url: &str, bind_address: IpAddr) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .hickory_dns(true)
            .local_address(Some(bind_address))
            .build()
            .unwrap();
        Self {
            http_client: client,
            url: url.to_string(),
            last_request_id: 0,
        }
    }

    fn get_id(&mut self) -> String {
        self.last_request_id += 1;
        self.last_request_id.to_string()
    }

    pub async fn request<T: DeserializeOwned, P: Serialize>(
        &mut self,
        method: &'static str,
        params: P,
    ) -> Result<T, error::Error> {
        let request_id = &self.get_id();
        let request = Request::new(request_id, method, params);
        let response = self
            .http_client
            .post(&self.url)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(error::Error::Http(status));
        }

        let text = response.text().await?;

        let response: Response<Value> = serde_json::from_str(&text)?;

        #[cfg(test)]
        eprintln!("Method `{}` called with response: {:#?}", method, response);

        match response {
            Response::Ok {
                jsonrpc: _,
                id: _,
                result,
            } => Ok(serde_json::from_value::<T>(result)?),

            Response::Err {
                jsonrpc: _,
                id: _,
                error,
            } => Err(error::Error::Rpc(error)),
        }
    }
}
