import { ChangeEvent, FormEvent, useState } from 'react'
import * as bitbox from 'bitbox-api';

import { ShowError } from './Error';

type Props = { bb02: bitbox.PairedBitBox };

const btcCoinOptions = ['btc', 'tbtc', 'ltc', 'tltc'];

function BtcXPub({ bb02 } : Props) {
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
    <div>
      <h4>XPub</h4>
      <form onSubmit={submitForm} className="verticalForm">
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
        <button type='submit' disabled={running}>Get XPub</button>
        {result ? <>
          <div className="resultContainer">
            <label>Result</label>
            {
              <textarea
              rows={result.split('\n').length + 2}
              readOnly
              defaultValue={result}
              />
            }
          </div>
        </> : null}
        <ShowError err={err} />
      </form>
    </div>
   
  );
}

function BtcAddressSimple({ bb02 }: Props) {
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
    <div>
      <h4>Address</h4>
      <form className="verticalForm" onSubmit={submitForm}>
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
          Display On Device
          <input type='checkbox' checked={display} onChange={(e: ChangeEvent<HTMLInputElement>) => setDisplay(e.target.checked)} />
        </label>
        <p>Keypath: { getKeypath() }</p>
        <button type='submit' disabled={running}>Get address</button>
        {result ? (
          <div className="resultContainer">
            <label>Result: <b><code>{result}</code></b></label>
          </div>
        ) : null }
        <ShowError err={err} />
      </form>
    </div>
  );
}

function BtcSignPSBT({ bb02 }: Props) {
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
    <div> 
      <h4>Sign PSBT</h4>
      <form className="verticalForm" onSubmit={submitForm}>
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
        {
          result ? (
            <div className="resultContainer">
              <label>Result: <b><code>{result}</code></b></label>
            </div>
          ) : null
        }
        <ShowError err={err} />
      </form>
    </div>
  );
}

function BtcSignMessage({ bb02 }: Props) {
  const [simpleType, setSimpleType] = useState<bitbox.BtcSimpleType>('p2wpkhP2sh');
  const [keypath, setKeypath] = useState("m/49'/0'/0'/0/0");
  const [message, setMessage] = useState('message');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<bitbox.BtcSignMessageSignature | undefined>();
  const [err, setErr] = useState<bitbox.Error>();

  const coin = 'btc';
  const simpleTypeOptions: bitbox.BtcSimpleType[] = ['p2wpkhP2sh', 'p2wpkh'];

  const scriptConfig: bitbox.BtcScriptConfig = { simpleType };
  const scriptConfigWKeypath: bitbox.BtcScriptConfigWithKeypath = { scriptConfig, keypath };

  const stringToUint8Array = (str: string) => {
    const arr = new Uint8Array(str.length);
    for (let i = 0; i < str.length; i++) {
        arr[i] = str.charCodeAt(i);
    }
    return arr;
  }

  const parsedResult = result ? JSON.stringify(result, undefined, 2) : '';

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      const signature = await bb02.btcSignMessage(coin, scriptConfigWKeypath, stringToUint8Array(message));
      setResult(signature);
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div>
      <h4>Sign Message</h4>
      <form className="verticalForm" onSubmit={submitForm}>
        <label>
          <p>Coin: { coin }</p>
        </label>
        <label>
          Simple Type
          <select value={simpleType} onChange={(e: ChangeEvent<HTMLSelectElement>) => setSimpleType(e.target.value as bitbox.BtcSimpleType)}>
            {simpleTypeOptions.map(option => <option key={option} value={option} disabled={false}>{option}</option>)}
          </select>
        </label>
        <label>
          Keypath
        </label>
        <input type="string" value={keypath} onChange={e => setKeypath(e.target.value)} />
        <label>
          Message
        </label>
        <textarea value={message} onChange={e => setMessage(e.target.value)} rows={4} cols={80} />
        <button type='submit' disabled={running}>Sign message</button>
        {result ? (
          <div className="resultContainer">
            <label>Result: 
            {
              <textarea
                rows={32}
                readOnly
                defaultValue={parsedResult}
              />
            }
            </label>
          </div>
        ) : null }
        <ShowError err={err} />
      </form>
    </div>
  );

}

function BtcMiniscriptAddress({ bb02 }: Props) {
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
    <div>
      <h4>Miniscript</h4>
      <form className="verticalForm" onSubmit={submitForm}>
        Address for policy <pre><code>{policy}</code></pre> using the BitBox02 xpub at
        <pre>{keypath}</pre>
        <p>and some other arbitrary xpub: <code>{someXPub}</code></p>
        <button type='submit' disabled={running}>Miniscript address</button>
        {result ? (
            <div className="resultContainer">
              <label>Result: <b><code>{result}</code></b></label>
            </div>
        ) : null }
        <ShowError err={err} />
      </form>
    </div>
  );
}

export function Bitcoin({ bb02 } : Props) {
  return (
    <>
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
        <BtcSignMessage bb02={bb02} />
      </div>
      <div className="action">
        <BtcMiniscriptAddress bb02={bb02} />
      </div>
    </>
  );
}
