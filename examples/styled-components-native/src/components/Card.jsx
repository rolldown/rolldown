import React from 'react';
import styled from 'styled-components';

const CardContainer = styled.div`
  background: white;
  border-radius: 12px;
  padding: 2rem;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1);
  transition: all 0.3s ease;
  backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.2);

  &:hover {
    transform: translateY(-5px);
    box-shadow: 0 15px 35px rgba(0, 0, 0, 0.15);
  }
`;

const CardTitle = styled.h3`
  margin: 0 0 1.5rem 0;
  color: #2d3748;
  font-size: 1.5rem;
  font-weight: 700;
`;

const CardContent = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
`;

const Card = ({ title, children }) => {
  return (
    <CardContainer>
      <CardTitle>{title}</CardTitle>
      <CardContent>{children}</CardContent>
    </CardContainer>
  );
};

export default Card;
