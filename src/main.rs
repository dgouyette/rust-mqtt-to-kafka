use std::{env, process, thread};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use ctrlc;
use kafka::producer::AsBytes;
use paho_mqtt as mqtt;
use uuid::Uuid;

use rdkafka::config::ClientConfig;
use rdkafka::message::{OwnedHeaders};
use rdkafka::producer::{BaseProducer, BaseRecord, FutureProducer, FutureRecord, Producer};
use rdkafka::util::get_rdkafka_version;

fn try_reconnect(cli: &mqtt::Client) -> bool {
    println!("Connection lost. Waiting to retry connection");
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(5000));
        if cli.reconnect().is_ok() {
            println!("Successfully reconnected");
            return true;
        }
    }
    println!("Unable to reconnect after several attempts.");
    false
}



fn main() {
    let host = env::args()
        .nth(1)
        .unwrap_or_else(|| "mqtt://192.168.1.29:1883".to_string());

    println!("{host}");
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(host)
        .client_id("rust_sync_consumer")
        .finalize();

    let cli = mqtt::Client::new(create_opts).unwrap_or_else(|e| {
        println!("Error creating the client: {:?}", e);
        process::exit(1);
    });

    let rx = cli.start_consuming();

    let lwt = mqtt::MessageBuilder::new()
        .topic("Production")
        .payload("Sync consumer lost connection")
        .finalize();

    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(false)
        .will_message(lwt)
        .finalize();

    println!("Connecting to the MQTT broker...");
    let subscriptions = ["Production/#", "Consommation/#"];


    let qos = [1, 1];

    match cli.connect(conn_opts) {
        Ok(rsp) => {
            if let Some(conn_rsp) = rsp.connect_response() {
                println!(
                    "Connected to: '{}' with MQTT version {}",
                    conn_rsp.server_uri, conn_rsp.mqtt_version
                );

                println!("Subscribing to topics with requested QoS: {:?}...", qos);

                cli.subscribe_many(&subscriptions, &qos)
                    .and_then(|rsp| {
                        rsp.subscribe_many_response()
                            .ok_or(mqtt::Error::General("Bad response"))
                    })
                    .and_then(|vqos| {
                        println!("QoS granted: {:?}", vqos);
                        Ok(())
                    })
                    .unwrap_or_else(|err| {
                        println!("Error subscribing to topics: {:?}", err);
                        cli.disconnect(None).unwrap();
                        process::exit(1);
                    });
            }
        }
        Err(e) => {
            println!("Error connecting to the broker: {:?}", e);
            process::exit(1);
        }
    }

    let ctrlc_cli = cli.clone();
    ctrlc::set_handler(move || {
        ctrlc_cli.stop_consuming();
    })
        .expect("Error setting Ctrl-C handler");



    let producer: &BaseProducer = &ClientConfig::new()
        .set("bootstrap.servers", "192.168.1.29:9092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");


    let mapping = HashMap::from([
        ("Production/metrics/W", "production_w"),
        ("Consommation/metrics/ActivatePower", "consommation_w"),
        ("Consommation/metrics/ImportEnergy", "import_w"),
        ("Consommation/metrics/ExportEnergy", "export_w"),
    ]);
    let now = SystemTime::now().elapsed().unwrap().as_millis() as i64;

    println!("\nWaiting for messages on topics {:?}...", subscriptions);
    for msg in rx.iter() {
        if let Some(msg) = msg {
            if let Some(topic) = mapping.get(msg.topic()) {

                let id = Uuid::new_v4();

                let delivery_status = producer
                    .send(
                        BaseRecord::to(topic)
                            .payload(msg.payload())
                            .timestamp(now)
                            .key(id.as_bytes())
                    );
                println!("topic {}  msg {}", topic, msg.payload_str());
            }
        } else if cli.is_connected() || !try_reconnect(&cli) {
            break;
        }
    }
}
