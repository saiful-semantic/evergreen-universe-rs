use eg::EgResult;
use evergreen as eg;

fn main() -> EgResult<()> {
    let client = eg::init()?;

    let mut ses = client.session("opensrf.settings");

    ses.connect()?; // Optional

    let params = vec!["Hello", "World", "Pamplemousse"];

    let mut req = ses.request("opensrf.system.echo", params)?;

    // We anticipate multiple responses.  Collect them all!
    while let Some(resp) = req.recv()? {
        println!("Response: {}", resp.dump());
    }

    ses.disconnect()?; // Only required if connected

    // ------------------------------------------------------------------
    // One-off request and we only care about the 1st response.

    let value = "Hello, World, Pamplemousse";
    let response = client
        .send_recv_one("opensrf.settings", "opensrf.system.echo", value)?
        .unwrap();

    // Client responses are EgValue's
    let resp_str = response.as_str().unwrap();

    assert_eq!(resp_str, value);

    println!("Response: {resp_str}");

    Ok(())
}
