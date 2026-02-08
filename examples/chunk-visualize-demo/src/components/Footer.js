// Footer component

export class Footer {
  constructor() {
    this.year = new Date().getFullYear();
  }

  render() {
    console.log('Rendering footer');
    return `<footer><p>&copy; ${this.year} Demo App</p></footer>`;
  }
}
