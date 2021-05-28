## Procedure: 
- For injecting CSS with Javascript: See [this](https://gist.github.com/ebith/fa0381b8b386c349da4dd474957791f9)
- Find file core.asar in directory %LOCALAPPDATA%\Discord\app-1.0.9001\modules\discord_desktop_core-1\discord_desktop_core
- - Make a backup of the file if requested
- Find the string 
```js 
mainWindow.webContents.
``` 
in the file and replace it with:
```js
mainWindow.webContents.on('dom-ready', () => {{
        mainWindow.webContents.executeJavaScript(`
            let CSS_INJECTION_USER_CSS = String.raw \\`{css}\\`;  
            const style = document.createElement('style');  
            style.innerHTML = CSS_INJECTION_USER_CSS;  
            document.head.appendChild(style);  
              
            //JS_SCRIPT_BEGIN 
            **{js}** 
            //JS_SCRIPT_END 
        `);
    }});mainWindow.webContents.
```
  - Ensure that the replacement has not already happened
- Save the core.asar file again
- Reload Discord

