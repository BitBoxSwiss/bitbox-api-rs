import { ChangeEvent, FormEvent, useState } from 'react'
import './App.css'
import * as bitbox from "bitbox-api";

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
  const [simpleType, setSimpleType] = useState<bitbox.BtcSimpleType>('p2wpkh-p2sh');
  const [display, setDisplay] = useState(true);
  const [isChange, setIsChange] = useState(false);
  const [running, setRunning] = useState(false);
  const [addressIndex, setAddressIndex] = useState(0);
  const [account, setAccount] = useState(0);
  const [result, setResult] = useState('');
  const [err, setErr] = useState<bitbox.Error>();

  const simpleTypeOptions: bitbox.BtcSimpleType[] = ['p2wpkh-p2sh', 'p2wpkh', 'p2tr'];

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
      case 'p2wpkh-p2sh': return 49;
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

function App() {
  const [bb02, setBB02] = useState<bitbox.PairedBitBox>();
  const [pairingCode, setPairingCode] = useState<string>();
  const [err, setErr] = useState<bitbox.Error>();
  const connectWebHID = async () => {
    setErr(undefined);
    try {
      const b = await bitbox.bitbox02ConnectWebHID();
      const pairing = await b.unlockAndPair();
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
        <h2>Connected</h2>
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
      </>
    );
  }
  return (
    <>
      <h1>BitBox sandbox</h1>
      <button onClick={connectWebHID}>Connect with WebHID</button>
    </>
  );
}

export default App
