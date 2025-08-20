import React from 'react';
import styled from 'styled-components';
import Button from './components/Button';
import Card from './components/Card';

const AppContainer = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 2rem;
  min-height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
`;

const Title = styled.h1`
  color: white;
  font-size: 3rem;
  margin-bottom: 2rem;
  text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.3);
`;

const ComponentsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 2rem;
  width: 100%;
  max-width: 1200px;
`;

function App() {
  return (
    <AppContainer>
      <Title>Styled Components with Rolldown</Title>
      <ComponentsGrid>
        <Card title='Primary Button'>
          <Button variant='primary' onClick={() => alert('Primary clicked!')}>
            Primary Button
          </Button>
        </Card>
        <Card title='Secondary Button'>
          <Button
            variant='secondary'
            onClick={() => alert('Secondary clicked!')}
          >
            Secondary Button
          </Button>
        </Card>
        <Card title='Disabled Button'>
          <Button disabled onClick={() => alert('This should not fire')}>
            Disabled Button
          </Button>
        </Card>
      </ComponentsGrid>
    </AppContainer>
  );
}

export default App;
