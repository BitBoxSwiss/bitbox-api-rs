import * as bitbox from 'bitbox-api';
import { ErrorNotification } from './ErrorNotification';
import { useEffect, useState } from 'react';

export function ShowError({ err }: { err?: bitbox.Error }) {
  const [error, setError] = useState<bitbox.Error>();

  useEffect(() => {
    setError(err);
  }, [err]);

  if (error === undefined) {
    return null;
  }

  return (
    <ErrorNotification
      message={error.message}
      code={error.code}
      onClose={() => setError(undefined)}
    />
  );
}
