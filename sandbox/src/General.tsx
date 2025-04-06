import { FormEvent, useState } from 'react'
import * as bitbox from 'bitbox-api';

import { ShowError } from './Error';

type Props = { bb02: bitbox.PairedBitBox };

function RootFingerprint({ bb02 } : Props) {
   const [rootFingerprint, setRootFingerprint] = useState<string>();

  const actionRootFingerprint = async (e: FormEvent) => {
    e.preventDefault();
    setRootFingerprint(undefined);
    setRootFingerprint(await bb02.rootFingerprint());
  };

  return (
    <>
      <h4>Root Fingerprint</h4>
      <button onClick={actionRootFingerprint}>Show</button>
      {rootFingerprint ? (
        <div className="resultContainer">
          <label>Result: <b><code>{rootFingerprint}</code></b></label>
        </div>
      ) : null}
    </>
  );
}

function DeviceInfo({ bb02 } : Props) {
  const [deviceInfo, setDeviceInfo] = useState<bitbox.DeviceInfo>();

  const actionDeviceInfo = async (e: FormEvent) => {
    e.preventDefault();
    setDeviceInfo(undefined);
    setDeviceInfo(await bb02.deviceInfo());
  };

  const parsedDeviceInfo = deviceInfo ? JSON.stringify(deviceInfo, undefined, 2) : '';

  return (
    <>
      <h4>Device Info</h4>
      <button onClick={actionDeviceInfo}>Show</button>
      {deviceInfo ? (
        <div className="resultContainer">
          <label>Result</label>
          {<textarea
            rows={parsedDeviceInfo.split('\n').length}
            readOnly
            defaultValue={parsedDeviceInfo}
          />}
        </div>
      ) : null}
    </>
  );
}

function ShowMnemonic({ bb02 } : Props) {
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
      <h4>Recovery Words</h4>
      <button onClick={actionShowMnemonic} disabled={running}>Show recovery words</button>
      <ShowError err={err} />
    </>
  );
}

function Bip85AppBip39({ bb02 } : Props) {
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const actionBip85 = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setErr(undefined);
    try {
      await bb02.bip85AppBip39();
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <>
      <h4>BIP-85</h4>
      <button onClick={actionBip85} disabled={running}>Invoke BIP-85 (BIP-39 app)</button>
      <ShowError err={err} />
    </>
  );
}

export function General({ bb02 } : Props) {
  return (
    <>
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
        <Bip85AppBip39 bb02={bb02} />
      </div>
    </>
  );
}
