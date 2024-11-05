export const SignUpForm = (props) => {
	return <p class="signup">
		<label>Username: <input class="username" type="text"/></label>
		<label>Password: <input class="password" type="password"/></label>
		<div class="primary disabled">
			{props.buttonText}
		</div>
		<small>By signing up, you are agreeing to our <a href="/tos/">terms of service</a>.</small>
	</p>
}