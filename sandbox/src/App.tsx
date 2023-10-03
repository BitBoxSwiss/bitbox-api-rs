import { useState } from 'react'
import * as bitbox from 'bitbox-api';
import './App.css'

import { Bitcoin } from './Bitcoin';
import { Cardano } from './Cardano';
import { Ethereum } from './Ethereum';
import { General } from './General';
import { ErrorNotification } from './ErrorNotification';
import { Accordion } from './Accordion';

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

 
  if (pairingCode !== undefined) {
    return (
      <div className="container">
        <h2>Pairing code</h2>
        <pre>
          {pairingCode}
        </pre>
      </div>
    );
  }
  if (bb02 !== undefined) {
    return (
      <div className="contentContainer">
        <h2 style={{textAlign: 'left'}}>BitBox02 sandbox</h2>
        <h4 style={{textAlign: 'left'}}>Connected product: {bb02.product()}</h4>
        <Accordion opened title="General">
          <General bb02={bb02} />
        </Accordion>
        <Accordion title="Bitcoin">
          <Bitcoin bb02={bb02} />
        </Accordion>
        { bb02.ethSupported() ? (
            <Accordion title="Ethereum">
              <Ethereum bb02={bb02} />
            </Accordion>
        ) : null }
        { bb02.cardanoSupported() ? (
            <Accordion title="Cardano">
              <Cardano bb02={bb02} />
            </Accordion>
        ) : null }
      </div>
    );
  }
  return (
    <div className="container">
      <h1>BitBox sandbox</h1>
      <button className="menuButton" onClick={() => connect('webHID')}>Connect using WebHID</button><br />
      <button className="menuButton" onClick={() => connect('bridge')}>Connect using BitBoxBridge</button><br />
      <button className="menuButton" onClick={() => connect('auto')}>Choose automatically</button>
      {err !== undefined && <ErrorNotification err={JSON.stringify(err)} onClose={() => setErr(undefined)} /> }
    </div>
  );
}

export default App
