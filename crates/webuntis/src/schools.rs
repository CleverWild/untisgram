use crate::{Error, School, SchoolSearchResult, jsonrpc, params::FindSchoolParams};

// pub static mut LOCAL_ADDRESS: Option<IpAddr> = None;

pub fn get_client() -> jsonrpc::Client {
    const URL: &str = "https://mobile.webuntis.com/ms/schoolquery2";
    // if let Some(addr) = unsafe { LOCAL_ADDRESS.as_ref() } {
    //     jsonrpc::Client::new_with_bind_address(URL, *addr)
    // } else {
    jsonrpc::Client::new(URL)
    // }
}

/// Returns all schools matching the query or an empty vec if there are too many results.
pub async fn search(query: &str) -> Result<Vec<School>, Error> {
    let result = get_client()
        .request(
            "searchSchool",
            vec![FindSchoolParams::Search { search: query }],
        )
        .await; // добавлено await
    catch_too_many(result)
}

/// Retrieves a school by its id.
pub async fn get_by_id(id: &usize) -> Result<School, Error> {
    let result = get_client()
        .request(
            "searchSchool",
            vec![FindSchoolParams::ById { schoolid: id }],
        )
        .await; // добавлено await

    get_first(catch_too_many(result)?)
}

/// Retrieves a school by it's [`login_name`](School#structfield.login_name).
pub async fn get_by_name(name: &str) -> Result<School, Error> {
    let result = search(name).await?;

    get_first(result)
}

fn get_first(mut list: Vec<School>) -> Result<School, Error> {
    if list.is_empty() {
        Err(Error::NotFound)
    } else {
        Ok(list.swap_remove(0))
    }
}

fn catch_too_many(result: Result<SchoolSearchResult, Error>) -> Result<Vec<School>, Error> {
    match result {
        Ok(v) => Ok(v.schools),
        Err(Error::Rpc(err)) => {
            if err.code == jsonrpc::ErrorCode::TooManyResults.as_isize() {
                Ok(vec![])
            } else {
                Err(Error::Rpc(err))
            }
        }
        Err(err) => Err(err),
    }
}
