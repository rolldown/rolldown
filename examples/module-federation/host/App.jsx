import RolldownButton from 'rolldown/button';
import WebpackButton from 'webpack/button';

export default function App() {
  return (
    <div className='App'>
      <header className='App-header'>
        <h1>Hello Rolldown + Module Federation</h1>
        <div>
          <h2>Rolldown Remote</h2>
          <RolldownButton />
        </div>
        <div>
          <h2>Webpack Remote</h2>
          <WebpackButton />
        </div>
      </header>
    </div>
  );
}
