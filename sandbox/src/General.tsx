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
      <button onClick={actionRootFingerprint}>Root fingerprint</button>
      { rootFingerprint ? (<> Result: { rootFingerprint }</>) : null }
    </>
  );
}

function DeviceInfo({ bb02 } : Props) {
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
      <button onClick={actionShowMnemonic} disabled={running}>Show recovery words</button>
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
    </>
  );
}
