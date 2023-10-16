import { useEffect } from "react";
import "./ErrorNotification.css";

type TProps = {
    err: string;
    onClose: () => void;
}

export const ErrorNotification = ({ err, onClose }: TProps) => {
  
  useEffect(() => {
    setTimeout(() => {
      onClose();
    }, 3500)
  }, [err, onClose]);

  return (
      <div className="alert">
          <div className="headerContainer">
            <h2>Error</h2>
            <button onClick={onClose}>X</button>
          </div>
          <pre>{err}</pre>
      </div>
  )
}
