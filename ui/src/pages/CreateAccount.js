import { useState } from 'react';
import { Button, Input, Label, Header } from 'semantic-ui-react';

const MIN_PASSWORD_LENGTH = 8;

const CreateAccount = ({ recoveryPhrase, sendPassword, endInitialConnectionPhase }) => {
  const [password, setPassword] = useState('');
  const [createdAccount, setCreatedAccount] = useState(false);

  const isValid = (password) => {
    console.log("[INFO]: Check password validity.")
    return password.length >= MIN_PASSWORD_LENGTH;
  };

  const onClickCreateAccount = async () => {
    console.log("[INFO]: Creating account.")
    if (isValid(password)) {
      await sendPassword(password);
      setPassword('');
      setCreatedAccount(true);
    }
  };

  const onClickConfirmRecoveryPhrase = async () => {
    console.log("[INFO]: Confirming recovery phrase.")
    await endInitialConnectionPhase();
  };

  return (
    <>
      {!createdAccount && (
        <>
          <Header> Create Account </Header>
          <Input
            type="password"
            label="Password"
            onChange={(e) => setPassword(e.target.value)}
          />
          <Button className="button" onClick={onClickCreateAccount}>
            Create Account
          </Button>
          {password.length > 0 && !isValid(password) && (<><br/><Label basic color='red' pointing> `Please enter a minimum of ${MIN_PASSWORD_LENGTH} characters.`</Label></>)}
        </>
      )}
      {createdAccount && (
        <>
          <Header className="recovery-phrase-header">
            Recovery Phrase
          </Header>
          <div className="recovery-phrase-info">
            <p>This phrase can restore your funds if you lose access to your account.</p>
            <p>Write it down on paper and store it somewhere secure.</p>
          </div>
          <div className="recovery-phrase-warning">
            ⚠️  Never share your recovery phrase with anyone! ⚠️ 
          </div>
          <div className="recovery-phrase">
            <b>{recoveryPhrase}</b>
          </div>
          <Button className="button" onClick={onClickConfirmRecoveryPhrase}>
            I have written down my recovery phrase.
          </Button>
        </>
      )}
    </>
  );
};

export default CreateAccount;
