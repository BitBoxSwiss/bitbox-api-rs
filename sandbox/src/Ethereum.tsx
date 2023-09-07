import { FormEvent, useState } from 'react'
import * as bitbox from 'bitbox-api';
import hexToArrayBuffer from 'hex-to-array-buffer'

import { ShowError } from './Error';

type Props = { bb02: bitbox.PairedBitBox };

function EthXPub({ bb02 } : Props) {
  const [keypath, setKeypath] = useState('m/44\'/60\'/0\'/0');
  const [result, setResult] = useState('');
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const xpub = await bb02.ethXpub(keypath);
      setResult(xpub);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  const keypaths = ['m/44\'/60\'/0\'/0', 'm/44\'/1\'/0\'/0'];

  return (
    <form onSubmit={submitForm}>
      <label>
        Keypath
        <select value={keypath} onChange={e => setKeypath(e.target.value)}>
          {keypaths.map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <button type='submit' disabled={running}>Get xpub</button>
      { result ? <p>Result: {result}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function EthAddress({ bb02 } : Props) {
  const [chainID, setChainID] = useState(1);
  const [keypath, setKeypath] = useState('m/44\'/60\'/0\'/0/0');
  const [display, setDisplay] = useState(true);
  const [result, setResult] = useState('');
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const xpub = await bb02.ethAddress(BigInt(chainID), keypath, display);
      setResult(xpub);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        chainID
        <input type='number' value={chainID} onChange={e => setChainID(parseInt(e.target.value))} />
      </label>
      <label>
        Keypath
        <input type='text' value={keypath} onChange={e => setKeypath(e.target.value)} />
      </label>
      <label>
        Display
        <input type='checkbox' checked={display} onChange={e => setDisplay(e.target.checked)} />
      </label>
      <button type='submit' disabled={running}>Get address</button>
      { result ? <p>Result: {result}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function EthSignTransaction({ bb02 } : Props) {
  const [chainID, setChainID] = useState(1);
  const [keypath, setKeypath] = useState('m/44\'/60\'/0\'/0/0');
  const defaultTx = `{
  "nonce": "1fdc",
  "gasPrice": "0165a0bc00",
  "gasLimit": "5208",
  "recipient": "04f264cf34440313b4a0192a352814fbe927b885",
  "value": "075cf1259e9c4000",
  "data": ""
}`;
  const [txJson, setTxJson] = useState(defaultTx);
  const [result, setResult] = useState<bitbox.EthSignature | undefined>();
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      const parsed = JSON.parse(txJson);
      const tx = {
        nonce: new Uint8Array(hexToArrayBuffer(parsed.nonce)),
        gasPrice: new Uint8Array(hexToArrayBuffer(parsed.gasPrice)),
        gasLimit: new Uint8Array(hexToArrayBuffer(parsed.gasLimit)),
        recipient: new Uint8Array(hexToArrayBuffer(parsed.recipient)),
        value: new Uint8Array(hexToArrayBuffer(parsed.value)),
        data: new Uint8Array(hexToArrayBuffer(parsed.data)),
      };
      setResult(await bb02.ethSignTransaction(BigInt(chainID), keypath, tx));
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        chainID
        <input type='number' value={chainID} onChange={e => setChainID(parseInt(e.target.value))} />
      </label>
      <label>
        Keypath
        <input type='text' value={keypath} onChange={e => setKeypath(e.target.value)} />
      </label>
      <br />
      <label>
        Transaction
        <textarea value={txJson} onChange={e => setTxJson(e.target.value)} rows={20} cols={80} />
      </label>
      <br />
      <button type='submit' disabled={running}>Sign transaction</button>
      { result ? <p>Result: {JSON.stringify(result)}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function EthSignMessage({ bb02 } : Props) {
  const [chainID, setChainID] = useState(1);
  const [keypath, setKeypath] = useState('m/44\'/60\'/0\'/0/0');
  const [msg, setMsg] = useState('message');
  const [result, setResult] = useState<bitbox.EthSignature | undefined>();
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const stringToUint8Array = (str: string) => {
    const arr = new Uint8Array(str.length);
    for (let i = 0; i < str.length; i++) {
        arr[i] = str.charCodeAt(i);
    }
    return arr;
  }

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      setResult(await bb02.ethSignMessage(BigInt(chainID), keypath, stringToUint8Array(msg)));
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        chainID
        <input type='number' value={chainID} onChange={e => setChainID(parseInt(e.target.value))} />
      </label>
      <label>
        Keypath
        <input type='text' value={keypath} onChange={e => setKeypath(e.target.value)} />
      </label>
      <br />
      <label>
        Message
        <textarea value={msg} onChange={e => setMsg(e.target.value)} rows={4} cols={80} />
      </label>
      <br />
      <button type='submit' disabled={running}>Sign message</button>
      { result ? <p>Result: {JSON.stringify(result)}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function EthSignTypedMessage({ bb02 } : Props) {
  const exampleMsg = `
  {
    "types": {
        "EIP712Domain": [
            { "name": "name", "type": "string" },
            { "name": "version", "type": "string" },
            { "name": "chainId", "type": "uint256" },
            { "name": "verifyingContract", "type": "address" }
        ],
        "Attachment": [
            { "name": "contents", "type": "string" }
        ],
        "Person": [
            { "name": "name", "type": "string" },
            { "name": "wallet", "type": "address" },
            { "name": "age", "type": "uint8" }
        ],
        "Mail": [
            { "name": "from", "type": "Person" },
            { "name": "to", "type": "Person" },
            { "name": "contents", "type": "string" },
            { "name": "attachments", "type": "Attachment[]" }
        ]
    },
    "primaryType": "Mail",
    "domain": {
        "name": "Ether Mail",
        "version": "1",
        "chainId": 1,
        "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
    },
    "message": {
        "from": {
            "name": "Cow",
            "wallet": "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826",
            "age": 20
        },
        "to": {
            "name": "Bob",
            "wallet": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB",
            "age": "0x1e"
        },
        "contents": "Hello, Bob!",
        "attachments": [{ "contents": "attachment1" }, { "contents": "attachment2" }]
    }
}
  `;
  const [chainID, setChainID] = useState(1);
  const [keypath, setKeypath] = useState('m/44\'/60\'/0\'/0/0');
  const [msg, setMsg] = useState(exampleMsg);
  const [result, setResult] = useState<bitbox.EthSignature | undefined>();
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      setResult(await bb02.ethSignTypedMessage(BigInt(chainID), keypath, JSON.parse(msg)));
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        chainID
        <input type='number' value={chainID} onChange={e => setChainID(parseInt(e.target.value))} />
      </label>
      <label>
        Keypath
        <input type='text' value={keypath} onChange={e => setKeypath(e.target.value)} />
      </label>
      <br />
      <label>
        EIP-712 typed message
        <textarea value={msg} onChange={e => setMsg(e.target.value)} rows={20} cols={80} />
      </label>
      <br />
      <button type='submit' disabled={running}>Sign message</button>
      { result ? <p>Result: {JSON.stringify(result)}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

export function Ethereum({ bb02 } : Props) {
  return (
    <>
      <div className="action">
        <EthXPub bb02={bb02} />
      </div>
      <div className="action">
        <EthAddress bb02={bb02} />
      </div>
      <div className="action">
        <EthSignTransaction bb02={bb02} />
      </div>
      <div className="action">
        <EthSignMessage bb02={bb02} />
      </div>
      <div className="action">
        <EthSignTypedMessage bb02={bb02} />
      </div>
    </>
  );
}
