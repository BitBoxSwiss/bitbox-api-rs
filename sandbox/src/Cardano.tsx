import { ChangeEvent, FormEvent, useState } from 'react'
import * as bitbox from 'bitbox-api';
import hexToArrayBuffer from 'hex-to-array-buffer'

import { ShowError } from './Error';

type Props = { bb02: bitbox.PairedBitBox };

function CardanoXpubs({ bb02 } : Props) {
  const [keypaths, setKeypaths] = useState(`["m/1852'/1815'/0'", "m/1852'/1815'/1'"]`);
  const [result, setResult] = useState<bitbox.CardanoXpubs | undefined>();
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<bitbox.Error>();

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      setResult(await bb02.cardanoXpubs(JSON.parse(keypaths)));
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }


  return (
    <div>
      <h4>XPubs</h4>
      <form className="verticalForm"onSubmit={submitForm}>
        <label>
          Keypaths
        </label>
        <textarea value={keypaths} onChange={e => setKeypaths(e.target.value)} rows={5} cols={80} />
        <button type='submit' disabled={running}>Get XPubs</button>
          {result ? <>
            <div className="resultContainer">
              <label>Result</label>
              {
                result.map((xpub, i) => (
                  <code key={i}>
                    {i}: <b>{ JSON.stringify(xpub) }</b><br />
                  </code>
                ))
              }
            </div>
          </> : null}
        <ShowError err={err} />
      </form>
    </div>
  );
}

function CardanoAddress({ bb02 }: Props) {
  const [network, setNetwork] = useState<bitbox.CardanoNetwork>('mainnet');
  const [display, setDisplay] = useState(true);
  const [isChange, setIsChange] = useState(false);
  const [running, setRunning] = useState(false);
  const [addressIndex, setAddressIndex] = useState(0);
  const [account, setAccount] = useState(0);
  const [result, setResult] = useState('');
  const [err, setErr] = useState<bitbox.Error>();

  const networkOptions = ['mainnet', 'testnet'];

  const getKeypathPayment = () => {
    return `m/1852'/1815'/${account}'/${isChange ? 1 : 0}/${addressIndex}`;
  }

  const getKeypathStake = () => {
    return `m/1852'/1815'/${account}'/2/0`;
  }

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult('');
    setErr(undefined);
    try {
      const config = {
        pkhSkh: {
          keypathPayment: getKeypathPayment(),
          keypathStake: getKeypathStake(),
        },
      };
      const address = await bb02.cardanoAddress(network, config, display);
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
      <form className="verticalForm"onSubmit={submitForm}>
        <label>
          Network
          <select value={network} onChange={(e: ChangeEvent<HTMLSelectElement>) => setNetwork(e.target.value as bitbox.CardanoNetwork)}>
            {networkOptions.map(option => <option key={option} value={option}>{option}</option>)}
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
        <p>Keypath payment: { getKeypathPayment() }</p>
        <p>Keypath stake: { getKeypathStake() }</p>
        <button type='submit' disabled={running}>Get address</button>
        {result ? (
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

function CardanoSignTransaction({ bb02 }: Props) {
  type TxType = 'normal' | 'zero-ttl' | 'tokens' | 'delegate' | 'vote-delegation' | 'vote-delegation-keyhash' | 'withdraw-staking-rewards';
  const [txType, setTxType] = useState<TxType>('normal');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<bitbox.CardanoSignTransactionResult | undefined>();
  const [err, setErr] = useState<bitbox.Error>();

  const parsedResult = result ? JSON.stringify(result, undefined, 2) : '';

  const txTypes = [
    ['normal', 'Normal transaction'],
    ['zero-ttl', 'Transaction with TTL=0'],
    ['tokens', 'Transaction sending tokens'],
    ['delegate', 'Delegate staking to a pool'],
    ['vote-delegation', 'Delegate vote to a dRep'],
    ['vote-delegation-keyhash', 'Delegate vote to a dRep with a keyhash'],
    ['withdraw-staking-rewards', 'Withdraw staking rewards'],
  ];

  const submitForm = async (e: FormEvent) => {
    e.preventDefault();
    setRunning(true);
    setResult(undefined);
    setErr(undefined);
    try {
      const network: bitbox.CardanoNetwork = 'mainnet';
      const inputs = [
        {
          keypath: "m/1852'/1815'/0'/0/0",
          prevOutHash: new Uint8Array(hexToArrayBuffer("59864ee73ca5d91098a32b3ce9811bac1996dcbaefa6b6247dcaafb5779c2538")),
          prevOutIndex: 0,
        },
      ];
      const changeConfig = {
        pkhSkh: {
          keypathPayment: "m/1852'/1815'/0'/1/0",
          keypathStake: "m/1852'/1815'/0'/2/0",
        },
      };

      const changeAddress = await bb02.cardanoAddress(network, changeConfig, false);
      const drepType: bitbox.CardanoDrepType = 'alwaysAbstain';
      const drepKeyHashType: bitbox.CardanoDrepType = 'keyHash';
      const transaction = () => {
        switch (txType) {
          case 'normal':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: 'addr1q9qfllpxg2vu4lq6rnpel4pvpp5xnv3kvvgtxk6k6wp4ff89xrhu8jnu3p33vnctc9eklee5dtykzyag5penc6dcmakqsqqgpt',
                  value: BigInt(1000000),
                },
                {
                  encodedAddress: changeAddress,
                  value: BigInt(4829501),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(170499),
              ttl: BigInt(41115811),
              certificates: [],
              withdrawals: [],
              validityIntervalStart: BigInt(41110811),
              allowZeroTTL: false,
            };
          case 'zero-ttl':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: 'addr1q9qfllpxg2vu4lq6rnpel4pvpp5xnv3kvvgtxk6k6wp4ff89xrhu8jnu3p33vnctc9eklee5dtykzyag5penc6dcmakqsqqgpt',
                  value: BigInt(1000000),
                },
                {
                  encodedAddress: changeAddress,
                  value: BigInt(4829501),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(170499),
              ttl: BigInt(0),
              certificates: [],
              withdrawals: [],
              validityIntervalStart: BigInt(41110811),
              allowZeroTTL: true,
            };
          case 'tokens':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: 'addr1q9qfllpxg2vu4lq6rnpel4pvpp5xnv3kvvgtxk6k6wp4ff89xrhu8jnu3p33vnctc9eklee5dtykzyag5penc6dcmakqsqqgpt',
                  value: BigInt(1000000),
                  assetGroups: [
                    //  Asset policy ids and asset names from: https://github.com/cardano-foundation/CIPs/blob/a2ef32d8a2b485fed7f6ffde2781dd58869ff511/CIP-0014/README.md#test-vectors
                    {
                      policyId: new Uint8Array(hexToArrayBuffer("1e349c9bdea19fd6c147626a5260bc44b71635f398b67c59881df209")),
                      tokens: [
                        {
                          assetName: new Uint8Array(hexToArrayBuffer("504154415445")),
                          value: BigInt(1),
                        },
                        {
                          assetName: new Uint8Array(hexToArrayBuffer("7eae28af2208be856f7a119668ae52a49b73725e326dc16579dcc373")),
                          value: BigInt(3),
                        },
                      ],
                    },
                  ],
                },
                {
                  encodedAddress: changeAddress,
                  value: BigInt(4829501),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(170499),
              ttl: BigInt(0),
              certificates: [],
              withdrawals: [],
              validityIntervalStart: BigInt(41110811),
              allowZeroTTL: false,
            };
          case 'delegate':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: changeAddress,
                  value: BigInt(2741512),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(191681),
              ttl: BigInt(41539125),
              certificates: [
                {
                  stakeRegistration: {
                    keypath: "m/1852'/1815'/0'/2/0",
                  },
                },
                {
                  stakeDelegation: {
                    keypath: "m/1852'/1815'/0'/2/0",
                    poolKeyhash: new Uint8Array(hexToArrayBuffer("abababababababababababababababababababababababababababab")),
                  },
                },
              ],
              withdrawals: [],
              validityIntervalStart: BigInt(41110811),
              allowZeroTTL: false,
            };
          case 'vote-delegation':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: changeAddress,
                  value: BigInt(2741512),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(191681),
              ttl: BigInt(41539125),
              certificates: [
                {
                  voteDelegation: {
                    keypath: "m/1852'/1815'/0'/2/0",
                    type: drepType,
                  },
                },
              ],
              withdrawals: [],
              validityIntervalStart: BigInt(41110811),
              allowZeroTTL: false,
            };
            case 'vote-delegation-keyhash':
              return {
                network,
                inputs,
                outputs: [
                  {
                    encodedAddress: changeAddress,
                    value: BigInt(2741512),
                    scriptConfig: changeConfig,
                  },
                ],
                fee: BigInt(191681),
                ttl: BigInt(41539125),
                certificates: [
                  {
                    voteDelegation: {
                      keypath: "m/1852'/1815'/0'/2/0",
                      type: drepKeyHashType,
                      drepCredhash: new Uint8Array(hexToArrayBuffer("abababababababababababababababababababababababababababab")),
                    },
                  },
                ],
                withdrawals: [],
                validityIntervalStart: BigInt(41110811),
                allowZeroTTL: false,
              };
          case 'withdraw-staking-rewards':
            return {
              network,
              inputs,
              outputs: [
                {
                  encodedAddress: changeAddress,
                  value: BigInt(4817591),
                  scriptConfig: changeConfig,
                },
              ],
              fee: BigInt(175157),
              ttl: BigInt(41788708),
              certificates: [],
              withdrawals: [
                {
                  keypath: "m/1852'/1815'/0'/2/0",
                  value: BigInt(1234567),
                },
              ],
              validityIntervalStart: BigInt(0),
              allowZeroTTL: false,
            };
        }
      };
      setResult(await bb02.cardanoSignTransaction(transaction()));
    } catch (err) {
      setErr(bitbox.ensureError(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div>
      <h4>Sign Transaction</h4>
      <form className="verticalForm"onSubmit={submitForm}>
        <label>
          <select value={txType} onChange={(e: ChangeEvent<HTMLSelectElement>) => setTxType(e.target.value as TxType)}>
            {txTypes.map(option => <option key={option[0]} value={option[0]}>{option[1]}</option>)}
          </select>
        </label>
        <button type='submit' disabled={running}>Sign transaction</button>
        {result ? <>
            <div className="resultContainer">
              <label>Result</label>
              {
                <textarea
                rows={32}
                readOnly
                defaultValue={parsedResult}
                />
              }
            </div>
          </> : null}
        <ShowError err={err} />
      </form>
    </div>
  );
}

export function Cardano({ bb02 } : Props) {
  return (
    <>
      <div className="action">
        <CardanoXpubs bb02={bb02} />
      </div>
      <div className="action">
        <CardanoAddress bb02={bb02} />
      </div>
      <div className="action">
        <CardanoSignTransaction bb02={bb02} />
      </div>
    </>
  );
}
