import { useState } from 'react';
import './Accordion.css';

type TProps = {
    title: string;
    children: React.ReactNode;
    opened?: boolean;
}

export const Accordion = ({ title, children, opened }: TProps) => {
    const [isOpen, setIsOpen] = useState(opened);

    return (
        <div className="accordion">
            <div className="accordion-header" onClick={() => setIsOpen(!isOpen)}>
                <h3>{title}</h3>
                <span className="toggle-icon">
                    {isOpen ? '-' : '+'}
                </span>
            </div>
            {isOpen && <div className="accordion-content">{children}</div>}
        </div>
    );
};
