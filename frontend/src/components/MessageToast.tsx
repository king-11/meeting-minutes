import {useEffect, useState} from 'react';

interface MessageToastProps {
    message: string;
    type: 'success' | 'error' | 'info';
    show: boolean;
    setShow: (show: boolean) => void;
    duration?: number;
    isVisible?: boolean;
    onClose?: () => void;
}

export function MessageToast({ message, type, show, setShow, duration = 3000, isVisible, onClose }: MessageToastProps) {
    // Support both show/setShow pattern and isVisible/onClose pattern
    const shouldShow = isVisible !== undefined ? isVisible : show;
    const handleClose = onClose || (() => setShow && setShow(false));
    
    useEffect(() => {
        if (shouldShow) {
            const timer = setTimeout(() => {
                handleClose();
            }, duration);
            
            return () => clearTimeout(timer);
        }
    }, [shouldShow, duration, handleClose]); 
    
    return (
        shouldShow && (
            <span className={`${
                type === 'success' ? 'text-green-500' : 
                type === 'error' ? 'text-red-500' : 
                'text-blue-500'
            }`}>{message}</span>
        )
    );
}