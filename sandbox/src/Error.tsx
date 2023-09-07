import * as bitbox from 'bitbox-api';

export function ShowError({ err } : { err?: bitbox.Error }) {
  if (err === undefined) {
    return null;
  }

  return (
    <>
      Error: { JSON.stringify(err) }
    </>
  );
}
