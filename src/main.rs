use compound_error::CompoundError;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Uri};
use kv_log_macro::{error, info};
use std::net::SocketAddr;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    /// Example: http://my-server:80/
    #[structopt(short = "x", long)]
    proxy: String,

    /// Example: bob
    #[structopt(short, long)]
    user: String,

    /// Example: hunter2
    #[structopt(short, long)]
    pass: String,

    /// Example: "My special place"
    #[structopt(short, long)]
    realm: String,
}

#[derive(Debug, CompoundError)]
enum Error {
    Hyper(hyper::Error),
    IO(std::io::Error),
}

async fn proxy_pass(mut req: Request<Body>, opt: &Opt) -> Result<Response<Body>, Error> {
    let req_uri = req.uri().clone();

    let unauthorized = || {
        info!("request received", {
            uri: format!("{}", req_uri).as_str(),
            authorized: false,
        });

        Ok(Response::builder()
            .status(401)
            .header(
                "WWW-Authenticate",
                format!(r#"Basic realm="{}", charset="UTF-8""#, opt.realm),
            )
            .body(Body::empty())
            .expect("infallible response"))
    };

    let authorization = match req.headers_mut().remove("Authorization") {
        Some(header) => header,
        None => return unauthorized(),
    };

    let credentials = authorization
        .to_str()
        .ok()
        .and_then(|s| s.strip_prefix("Basic "))
        .and_then(|s| base64::decode(s).ok())
        .and_then(|b| String::from_utf8(b).ok());

    match credentials.as_ref().and_then(|s| s.split_once(":")) {
        Some(c) if c == (&opt.user, &opt.pass) => {}
        _ => return unauthorized(),
    }

    let proxy_uri: Uri = opt.proxy.parse().expect("proxy uri");
    let mut new_uri = Uri::builder().authority(proxy_uri.authority().unwrap().clone());
    new_uri = new_uri.scheme(proxy_uri.scheme_str().unwrap_or("http"));

    if let Some(paq) = req_uri.path_and_query().cloned() {
        new_uri = new_uri.path_and_query(paq);
    }

    *req.uri_mut() = new_uri.build().expect("uri");

    let client = Client::new();
    let mut error = None;
    let response = match client.request(req).await {
        Ok(response) => response,
        Err(e) => {
            error = Some(format!("{:?}", e));
            Response::builder()
                .status(503)
                .body("503 Service Unavailable".into())
                .expect("infallible response")
        }
    };

    info!("request received", {
        uri: format!("{}", req_uri).as_str(),
        authorized: true,
        error: format!("{:?}", error).as_str(),
    });

    Ok(response)
}

#[tokio::main]
async fn main() {
    let opt: &'static Opt = Box::leak(Box::new(Opt::from_args()));

    femme::start();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on 0.0.0.0:3000");

    let make_svc =
        make_service_fn(
            |_conn| async move { Ok::<_, Error>(service_fn(move |r| proxy_pass(r, opt))) },
        );

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }
}
