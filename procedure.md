## Procedure: 
- For injecting CSS: See [this](https://gist.github.com/ebith/fa0381b8b386c349da4dd474957791f9)
- Find file core.asar in directory %APPDATA%\Discord\app-1.0.9001\modules\discord_desktop_core-1\discord_desktop_core
- - Make a backup of the file if requested
- Find the string 
```js 
mainWindow.webContents.send(\`${DISCORD_NAMESPACE}${event}\`, ...options); 
``` 
in the file and replace it with:
```js
mainWindow.webContents.on('dom-ready', () => {
      mainWindow.webContents.executeJavaScript(`
          let userCss = \`**USER CSS**\`;
          const style = document.createElement('style');
          style.innerHTML = userCss;
          document.head.appendChild(style);
          
          **CUSTOM USER JAVASCRIPT**
          `);
    });mainWindow.webContents.send(`${DISCORD_NAMESPACE}${event}`, ...options);
```
  - Ensure that the replacement has not already happened
  - Allow the user to add additional javascript to the file?
- Save the core.asar file again
- Reload Discord