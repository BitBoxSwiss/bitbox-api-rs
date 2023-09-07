import { useState } from 'react'
import * as bitbox from 'bitbox-api';
import './App.css'

import { Bitcoin } from './Bitcoin';
import { Cardano } from './Cardano';
import { Ethereum } from './Ethereum';
import { General } from './General';

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
        <h3>General</h3>
        <General bb02={bb02} />
        <h3>Bitcoin</h3>
        <Bitcoin bb02={bb02} />
        { bb02.ethSupported() ? (
            <>
              <h3>Ethereum</h3>
              <Ethereum bb02={bb02} />
            </>
        ) : null }
        { bb02.cardanoSupported() ? (
            <>
              <h3>Cardano</h3>
              <Cardano bb02={bb02} />
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
