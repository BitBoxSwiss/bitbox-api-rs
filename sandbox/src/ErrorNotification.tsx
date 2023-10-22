import { useEffect } from "react";
import "./ErrorNotification.css";

type TProps = {
    message: string;
    code: string;
    onClose: () => void;
}

export const ErrorNotification = ({ message, code, onClose }: TProps) => {
  
  useEffect(() => {
    const timerId = setTimeout(() => {
      onClose();
    }, 3500);

    return () => clearTimeout(timerId);
  }, [message, onClose]);

  return (
      <div className="alert">
          <div className="headerContainer">
            <h2>Error</h2>
            <button onClick={onClose}>X</button>
          </div>
      <p>{message}</p>
      <kbd>code: {code}</kbd>
      </div>
  )
}
