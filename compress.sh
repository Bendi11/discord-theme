cp old.css old-compressed.css
sed "/\/\*.*\*\//d;/\/\*/,/\*\// d" old-compressed.css
