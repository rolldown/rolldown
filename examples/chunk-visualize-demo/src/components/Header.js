// Header component

export class Header {
  constructor() {
    this.title = 'Chunk Visualization Demo';
  }

  render() {
    console.log('Rendering header:', this.title);
    return `<header><h1>${this.title}</h1></header>`;
  }
}
