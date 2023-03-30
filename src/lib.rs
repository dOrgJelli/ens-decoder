pub mod wrap;
pub use wrap::*;
use wrap::imported::*;

use lazy_static::lazy_static;
use regex::Regex;

pub fn decode(args: ArgsDecode) -> Option<String> {
    match args.tx_data.method.as_str() {
        "setAddr" => {
            Some(decode_set_addr(args.clone()).unwrap())
        },
        "setText" => {
            Some(decode_set_text(args.clone()).unwrap())
        },
        "multicall" => {
            Some(decode_multicall(args.clone()).unwrap())
        }
        _ => None
    }
}

fn decode_set_addr(args: ArgsDecode) -> Result<String, String> {
    // extract parameters
    let mut node: Option<String> = None;
    let mut coin_type: Option<String> = None;
    let mut address: Option<String> = None;

    for parameter in &args.tx_data.parameters.unwrap() {
        match parameter.name.as_str() {
            "node" => { node = Some(parameter.value.clone()) },
            "coinType" => { coin_type = Some(parameter.value.clone()) },
            "a" => { address = Some(parameter.value.clone()) },
            _ => { continue }
        }
    }

    // decode parameters
    let domain: String = decode_node(node.unwrap());
    let coin: String = decode_coin_type(coin_type.unwrap());

    // format into friendly response
    Ok(format!(
        "On the domain **{}**, set the **{}** blockchain address to **{}**.",
        domain, coin, address.unwrap()
    ).to_string())
}

fn decode_set_text(args: ArgsDecode) -> Result<String, String> {
    // extract parameters
    let mut node: Option<String> = None;
    let mut key: Option<String> = None;
    let mut value: Option<String> = None;

    for parameter in &args.tx_data.parameters.unwrap() {
        match parameter.name.as_str() {
            "node" => { node = Some(parameter.value.clone()) },
            "key" => { key = Some(parameter.value.clone()) },
            "value" => { value = Some(parameter.value.clone()) },
            _ => { continue }
        }
    }

    // decode parameters
    let domain: String = decode_node(node.unwrap());

    // format into friendly response
    Ok(format!(
        "On the domain **{}**, set the text record **{}** to **{}**.",
        domain, key.unwrap(), value.unwrap()
    ).to_string())
}

fn decode_multicall(args: ArgsDecode) -> Result<String, String> {
    // extract parameters
    let mut data: Option<String> = None;
    let to = args.tx_data.to;

    for parameter in &args.tx_data.parameters.unwrap() {
        match parameter.name.as_str() {
            "data" => { data = Some(parameter.value.clone()) }
            _ => { continue }
        }
    }

    // extract the calldatas
    lazy_static! {
        static ref BYTES_REGEX : Regex = Regex::new(
            r"0x[a-fA-F0-9]+"
        ).unwrap();
    }
    let calldatas = BYTES_REGEX.find_iter(data.unwrap().as_str()).map(
        |mat| mat.as_str().to_string()
    ).collect::<Vec<String>>();

    let mut result = "A multi-step transaction is being performed.".to_string();
    let mut counter = 0;

    // decode the calldata
    for calldata in calldatas {
        counter += 1;
        result.push_str(&format!(" **Step {}**: ", counter));
        result.push_str(&decode_calldata(to.clone(), calldata));
    }

    Ok(result)
}

fn decode_node(
    node: String
) -> String {
    let subgraphs = [
        "ensdomains/ens",
        "ensdomains/ensgoerli"
    ];

    for subgraph in subgraphs {
        let url = format!("https://api.thegraph.com/subgraphs/name/{}", subgraph);
        let query_start = r#"query {
            domain(id: ""#;
        let query_end = r#"") {
                name
            }
        }"#;
        let query = format!("{}{}{}", query_start, node, query_end);

        let resp = SubgraphModule::query_subgraph(&ArgsQuerySubgraph {
            url: url,
            query: query
        }).unwrap();

        // extract the domain name
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r#""name":[\s]*"(?P<domain>[^\s]+)""#
            ).unwrap();
        }
        let domain = RE.captures(&resp.as_str()).and_then(|cap| {
            cap.name("domain").map(|m| m.as_str())
        });

        match domain {
            Some(domain) => { return domain.to_string(); },
            None => { continue; }
        }
    }

    node
}

fn decode_coin_type(
  coin_type: String
) -> String {
  let name = match coin_type.as_str() {
    "0" => { "Bitcoin" },
    "2" => { "Litecoin" },
    "22" => { "Monacoin" },
    "60" => { "Ethereum" },
    "61" => { "Ethereum Classic" },
    "137" => { "Rootstock" },
    "144" => { "Ripple" },
    "145" => { "Bitcoin Cash" },
    "714" => { "Binance" },
    // TODO: add other tokens from slip-0044
    // https://github.com/satoshilabs/slips/blob/master/slip-0044.md
    _ => { coin_type.as_str() }
  };
  name.to_string()
}

fn decode_calldata(
    to: String,
    calldata: String
) -> String {
    // Extract the function signature
    let sig = &calldata[0..10];
    let function = decode_function_signature(
        sig.to_string()
    ).unwrap();

    // Aggregate function argument types
    let mut arg_types = "".to_string();
    let arg_len = function.arg_types.len();

    for n in 0..arg_len {
        arg_types.push_str(
            function.arg_types[n]
        );

        if n + 1 < arg_len {
            arg_types.push_str(",");
        }
    }

    let method = format!(
        "function {}({})",
        function.name,
        arg_types
    );

    // Decode the function parameters
    let values = EthersUtilsModule::decode_function(&ArgsDecodeFunction {
        method: method,
        data: calldata
    }).unwrap();

    let mut parameters: Vec<Parameter> = Vec::new();

    for n in 0..arg_len {
        parameters.push(Parameter {
            name: function.arg_names[n].to_string(),
            _type: function.arg_types[n].to_string(),
            value: values[n].clone()
        });
    }

    decode(ArgsDecode {
        tx_data: TxData {
            to: to,
            method: function.name.to_string(),
            parameters: Some(parameters)
        }
    }).unwrap()
}

struct Function<'a> { 
    name: &'a str,
    arg_types: Vec<&'a str>,
    arg_names: Vec<&'a str>
}

fn decode_function_signature(
    signature: String
) -> Option<Function<'static>> {
    match signature.as_str() {
        "0xa4b91a01" => {
            Some(Function {
                name: "approve",
                arg_types: vec!("bytes32", "address", "bool"),
                arg_names: vec!("node", "delegate", "approved")
            })
        },
        "0x3603d758" => {
            Some(Function {
                name: "clearRecords",
                arg_types: vec!("bytes32"),
                arg_names: vec!("node")
            })
        },
        "0x623195b0" => {
            Some(Function {
                name: "setABI",
                arg_types: vec!("bytes32", "uint256", "bytes"),
                arg_names: vec!("node", "contentType", "data")
            })
        },
        "0x8b95dd71" => {
            Some(Function {
                name: "setAddr",
                arg_types: vec!("bytes32", "uint256", "bytes"),
                arg_names: vec!("node", "coinType", "a")
            })
        },
        "0xd5fa2b00" => {
            Some(Function {
                name: "setAddr",
                arg_types: vec!("bytes32", "address"),
                arg_names: vec!("node", "a")
            })
        },
        "0xa22cb465" => {
            Some(Function {
                name: "setApprovalForAll",
                arg_types: vec!("address", "bool"),
                arg_names: vec!("operator", "approved")
            })
        },
        "0x304e6ade" => {
            Some(Function {
                name: "setContentHash",
                arg_types: vec!("bytes32", "bytes"),
                arg_names: vec!("node", "hash")
            })
        },
        "0x0af179d7" => {
            Some(Function {
                name: "setDNSRecords",
                arg_types: vec!("bytes32", "bytes"),
                arg_names: vec!("node", "data")
            })
        },
        "0xe59d895d" => {
            Some(Function {
                name: "setInterface",
                arg_types: vec!("bytes32", "bytes4", "address"),
                arg_names: vec!("node", "interfaceID", "implementer")
            })
        },
        "0x77372213" => {
            Some(Function {
                name: "setName",
                arg_types: vec!("bytes32", "string"),
                arg_names: vec!("node", "newName")
            })
        },
        "0x29cd62ea" => {
            Some(Function {
                name: "setPubkey",
                arg_types: vec!("bytes32", "bytes32", "bytes32"),
                arg_names: vec!("node", "x", "y")
            })
        },
        "0x10f13a8c" => {
            Some(Function {
                name: "setText",
                arg_types: vec!("bytes32", "string", "string"),
                arg_names: vec!("node", "key", "value")
            })
        },
        "0xce3decdc" => {
            Some(Function {
                name: "setZonehash",
                arg_types: vec!("bytes32", "bytes"),
                arg_names: vec!("node", "hash")
            })
        },
        _ => { None }
    }
}
