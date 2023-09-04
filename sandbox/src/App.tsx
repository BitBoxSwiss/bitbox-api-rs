import { ChangeEvent, FormEvent, useState } from 'react'
import './App.css'
import * as bitbox from 'bitbox-api';
import hexToArrayBuffer from 'hex-to-array-buffer'

type ActionProps = { bb02: bitbox.PairedBitBox };

const btcCoinOptions = ['btc', 'tbtc', 'ltc', 'tltc'];

function ShowError({ err } : { err?: bitbox.Error }) {
  if (err === undefined) {
    return null;
  }

  return (
    <>
      Error: { JSON.stringify(err) }
    </>
  );
}

function RootFingerprint({ bb02 } : ActionProps) {
   const [rootFingerprint, setRootFingerprint] = useState<string>();

  const actionRootFingerprint = async (e: FormEvent) => {
    e.preventDefault();
    setRootFingerprint(undefined);
    setRootFingerprint(await bb02.rootFingerprint());
  };

  return (
    <>
      <button onClick={actionRootFingerprint}>Root fingerprint</button>
      { rootFingerprint ? (<> Result: { rootFingerprint }</>) : null }
    </>
  );
}

function DeviceInfo({ bb02 } : ActionProps) {
  const [deviceInfo, setDeviceInfo] = useState<bitbox.DeviceInfo>();

  const actionDeviceInfo = async (e: FormEvent) => {
    e.preventDefault();
    setDeviceInfo(await bb02.deviceInfo());
  };

  return (
    <>
      <button onClick={actionDeviceInfo}>Device info</button>
      { deviceInfo ? (<> Result: { JSON.stringify(deviceInfo) }</>) : null }
    </>
  );
}

function ShowMnemonic({ bb02 } : ActionProps) {
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const actionShowMnemonic = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setErr(undefined);
    try {
      await bb02.showMnemonic();
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <>
      <button onClick={actionShowMnemonic} disabled={running}>Show recovery words</button>
      <ShowError err={err} />
    </>
  );
}

function BtcXPub({ bb02 } : ActionProps) {
  const [coin, setCoin] = useState<bitbox.BtcCoin>('btc');
  const [keypath, setKeypath] = useState('m/84\'/0\'/0\'');
  const [xpubType, setXpubType] = useState<bitbox.XPubType>('xpub');
  const [display, setDisplay] = useState(true);
  const [result, setResult] = useState('');
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const xpubTypeOptions = ['tpub', 'xpub', 'ypub', 'zpub', 'vpub', 'upub', 'Vpub', 'Zpub', 'Upub', 'Ypub'];

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const xpub = await bb02.btcXpub(coin, keypath, xpubType, display);
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
        Coin
        <select value={coin} onChange={e => setCoin(e.target.value as bitbox.BtcCoin)}>
          {btcCoinOptions.map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <label>
        Keypath
        <input type='text' value={keypath} onChange={e => setKeypath(e.target.value)} />
      </label>
      <label>
        XPub Type
        <select value={xpubType} onChange={e => setXpubType(e.target.value as bitbox.XPubType)}>
          {xpubTypeOptions.map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <label>
        Display
        <input type='checkbox' checked={display} onChange={e => setDisplay(e.target.checked)} />
      </label>
      <button type='submit' disabled={running}>Get xpub</button>
      { result ? <p>Result: {result}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function BtcAddressSimple({ bb02 }: ActionProps) {
  const [coin, setCoin] = useState<bitbox.BtcCoin>('btc');
  const [simpleType, setSimpleType] = useState<bitbox.BtcSimpleType>('p2wpkhP2sh');
  const [display, setDisplay] = useState(true);
  const [isChange, setIsChange] = useState(false);
  const [running, setRunning] = useState(false);
  const [addressIndex, setAddressIndex] = useState(0);
  const [account, setAccount] = useState(0);
  const [result, setResult] = useState('');
  const [err, setErr] = useState<bitbox.Error>();

  const simpleTypeOptions: bitbox.BtcSimpleType[] = ['p2wpkhP2sh', 'p2wpkh', 'p2tr'];

  const getCoin = (coin: bitbox.BtcCoin) => {
    switch (coin) {
      case 'btc': return 0;
      case 'tbtc':
      case 'tltc': return 1;
      case 'ltc': return 2;
    }
  }

  const getPurpose = (type: bitbox.BtcSimpleType) => {
    switch (type) {
      case 'p2wpkhP2sh': return 49;
      case 'p2wpkh': return 84;
      case 'p2tr': return 86;
    }
  }

  const getKeypath = () => {
    return `m/${getPurpose(simpleType)}'/${getCoin(coin)}'/${account}'/${isChange ? 1 : 0}/${addressIndex}`;
  }

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const address = await bb02.btcAddress(coin, getKeypath(), { simpleType }, display);
      setResult(address);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        Coin
        <select value={coin} onChange={(e: ChangeEvent<HTMLSelectElement>) => setCoin(e.target.value as bitbox.BtcCoin)}>
          {btcCoinOptions.map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <label>
        Simple Type
        <select value={simpleType} onChange={(e: ChangeEvent<HTMLSelectElement>) => setSimpleType(e.target.value as bitbox.BtcSimpleType)}>
          {simpleTypeOptions.map(option => <option key={option} value={option} disabled={option === 'p2tr' && (coin === 'ltc' || coin === 'tltc')}>{option}</option>)}
        </select>
      </label>
      <label>
        Account
        <input type='number' min='0' value={account} onChange={(e: ChangeEvent<HTMLInputElement>) => setAccount(Number(e.target.value))} />
      </label>
      <label>
        Change
        <input type='checkbox' checked={isChange} onChange={(e: ChangeEvent<HTMLInputElement>) => setIsChange(e.target.checked)} />
      </label>
      <label>
        Address Index
        <input type='number' min='0' value={addressIndex} onChange={(e: ChangeEvent<HTMLInputElement>) => setAddressIndex(Number(e.target.value))} />
      </label>
      <label>
        Display
          <input type='checkbox' checked={display} onChange={(e: ChangeEvent<HTMLInputElement>) => setDisplay(e.target.checked)} />
      </label>
      <p>Keypath: { getKeypath() }</p>
      <button type='submit' disabled={running}>Get address</button>
      { result ? <p>Result: {result}</p> : null }
      <ShowError err={err} />
    </form>
  );
}

function BtcSignPSBT({ bb02 }: ActionProps) {
  const [coin, setCoin] = useState<bitbox.BtcCoin>('btc');
  const [psbt, setPSBT] = useState<string>('');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState('');
  const [formatUnit, setFormatUnit] = useState<bitbox.BtcFormatUnit>('default');
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const signedPSBT = await bb02.btcSignPSBT(coin, psbt, undefined, formatUnit);
      setResult(signedPSBT);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
      <label>
        Coin
        <select value={coin} onChange={(e: ChangeEvent<HTMLSelectElement>) => setCoin(e.target.value as bitbox.BtcCoin)}>
          {btcCoinOptions.map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <label>
        Format unit
        <select value={formatUnit} onChange={(e: ChangeEvent<HTMLSelectElement>) => setFormatUnit(e.target.value as bitbox.BtcFormatUnit)}>
          {['default', 'sat'].map(option => <option key={option} value={option}>{option}</option>)}
        </select>
      </label>
      <label>
        PSBT
        <textarea
          value={psbt}
          onChange={(e: ChangeEvent<HTMLTextAreaElement>) => setPSBT(e.target.value)}
          placeholder="base64 PSBT"
        />
      </label>
      <button type='submit' disabled={running}>Sign PSBT</button>
      { result ? <pre>Result: <code>{result}</code></pre> : null }
      <ShowError err={err} />
    </form>
  );
}

function BtcMiniscriptAddress({ bb02 }: ActionProps) {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState('');
  const [err, setErr] = useState<bitbox.Error>();

  const coin = 'tbtc';
  const policy = "wsh(andor(pk(@0/**),older(12960),pk(@1/**)))";
  const keypath = "m/48'/1'/0'/3'";
  const someXPub = "tpubDFgycCkexSxkdZfeyaasDHityE97kiYM1BeCNoivDHvydGugKtoNobt4vEX6YSHNPy2cqmWQHKjKxciJuocepsGPGxcDZVmiMBnxgA1JKQk";

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const ourRootFingerprint = await bb02.rootFingerprint();
      const ourXPub = await bb02.btcXpub(coin, keypath, 'xpub', false);
      const keys = [
        {
          rootFingerprint: ourRootFingerprint,
          keypath,
          xpub: ourXPub,
        },
        {
          xpub: someXPub,
        },
      ];
      const scriptConfig = {
        policy: { policy, keys },
      };
      const is_script_config_registered = await bb02.btcIsScriptConfigRegistered(coin, scriptConfig, undefined);
      if (!is_script_config_registered) {
        await bb02.btcRegisterScriptConfig(coin, scriptConfig, undefined, 'autoXpubTpub', undefined);
      }
      const address = await bb02.btcAddress(coin, keypath + "/0/10", scriptConfig, true);
      setResult(address);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <form onSubmit={submitForm}>
    Address for policy <pre><code>{policy}</code></pre> using the BitBox02 xpub at
    <pre>{keypath}</pre> and some other arbitrary xpub: <pre><code>{someXPub}</code></pre>
    <button type='submit' disabled={running}>Miniscript address</button>
    { result ? <pre>Result: <code>{result}</code></pre> : null }
    <ShowError err={err} />
    </form>
  );
}

function EthXPub({ bb02 } : ActionProps) {
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

function EthAddress({ bb02 } : ActionProps) {
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

function EthSignTransaction({ bb02 } : ActionProps) {
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

function EthSignMessage({ bb02 } : ActionProps) {
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

function EthSignTypedMessage({ bb02 } : ActionProps) {
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

function App() {
  const [bb02, setBB02] = useState<bitbox.PairedBitBox>();
  const [pairingCode, setPairingCode] = useState<string>();
  const [err, setErr] = useState<bitbox.Error>();
  const onClose = () => {
    setBB02(undefined);
    setPairingCode(undefined);
    setErr(undefined);
  };
  const connect = async (method: 'webHID' | 'bridge' | 'auto') => {
    setErr(undefined);
    try {
      let device: bitbox.BitBox;
      switch (method) {
        case 'webHID':
          device = await bitbox.bitbox02ConnectWebHID(onClose);
          break;
        case 'bridge':
          device = await bitbox.bitbox02ConnectBridge(onClose);
          break;
        case 'auto':
          device = await bitbox.bitbox02ConnectAuto(onClose);
          break;
      }
      const pairing = await device.unlockAndPair();
      setPairingCode(pairing.getPairingCode());
      setBB02(await pairing.waitConfirm());
      setPairingCode(undefined);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    }
  };

  if (err !== undefined) {
     return (
      <>
        <h2>Error</h2>
        <pre>
          { JSON.stringify(err) }
        </pre>
      </>
    );
  }
  if (pairingCode !== undefined) {
    return (
      <>
        <h2>Pairing code</h2>
        <pre>
          { pairingCode }
        </pre>
      </>
    );
  }
  if (bb02 !== undefined) {
    return (
      <>
        <h2>Connected. Product: {bb02.product()}</h2>
        <h3>Bitcoin</h3>
        <div className="action">
          <RootFingerprint bb02={bb02} />
        </div>
        <div className="action">
          <DeviceInfo bb02={bb02} />
        </div>
        <div className="action">
          <ShowMnemonic bb02={bb02} />
        </div>
        <div className="action">
          <BtcXPub bb02={bb02} />
        </div>
        <div className="action">
          <BtcAddressSimple bb02={bb02} />
        </div>
        <div className="action">
          <BtcSignPSBT bb02={bb02} />
        </div>
        <div className="action">
          <BtcMiniscriptAddress bb02={bb02} />
        </div>
        { bb02.ethSupported() ? (
            <>
              <h3>Ethereum</h3>
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
        ) : null }
      </>
    );
  }
  return (
    <>
      <h1>BitBox sandbox</h1>
      <button onClick={() => connect('webHID')}>Connect using WebHID</button><br />
      <button onClick={() => connect('bridge')}>Connect using BitBoxBridge</button><br />
      <button onClick={() => connect('auto')}>Choose automatically</button>
    </>
  );
}

export default App
