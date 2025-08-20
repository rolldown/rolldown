import React from 'react';
import styled, { css } from 'styled-components';

const StyledButton = styled.button`
  padding: 0.75rem 1.5rem;
  border-radius: 8px;
  border: none;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease-in-out;
  position: relative;
  overflow: hidden;

  &:focus {
    outline: 2px solid #4299e1;
    outline-offset: 2px;
  }

  &:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  /* Hover effect ripple */
  &::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 0;
    height: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.3);
    transition: width 0.6s, height 0.6s, top 0.6s, left 0.6s;
    transform: translate(-50%, -50%);
    z-index: 0;
  }

  &:hover:not(:disabled)::before {
    width: 300px;
    height: 300px;
  }

  /* Content positioning */
  & > * {
    position: relative;
    z-index: 1;
  }

  ${props =>
  props.variant === 'primary' &&
  css`
      background: linear-gradient(45deg, #4299e1, #3182ce);
      color: white;
      box-shadow: 0 4px 14px rgba(66, 153, 225, 0.39);

      &:hover:not(:disabled) {
        transform: translateY(-2px);
        box-shadow: 0 6px 20px rgba(66, 153, 225, 0.6);
      }

      &:active:not(:disabled) {
        transform: translateY(0);
      }
    `}

  ${props =>
  props.variant === 'secondary' &&
  css`
      background: linear-gradient(45deg, #718096, #4a5568);
      color: white;
      box-shadow: 0 4px 14px rgba(113, 128, 150, 0.39);

      &:hover:not(:disabled) {
        transform: translateY(-2px);
        box-shadow: 0 6px 20px rgba(113, 128, 150, 0.6);
      }

      &:active:not(:disabled) {
        transform: translateY(0);
      }
    `}

  ${props =>
  !props.variant &&
  css`
      background: linear-gradient(45deg, #e2e8f0, #cbd5e0);
      color: #2d3748;
      box-shadow: 0 4px 14px rgba(226, 232, 240, 0.39);

      &:hover:not(:disabled) {
        transform: translateY(-2px);
        box-shadow: 0 6px 20px rgba(226, 232, 240, 0.6);
      }

      &:active:not(:disabled) {
        transform: translateY(0);
      }
    `}
`;

const Button = ({ children, variant = 'default', ...props }) => {
  return (
    <StyledButton variant={variant} {...props}>
      {children}
    </StyledButton>
  );
};

export default Button;
