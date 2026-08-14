# Sirve los archivos estáticos del IDE con nginx
FROM nginx:alpine
COPY . /usr/share/nginx/html
RUN cp "/usr/share/nginx/html/IDE Analizador Sintactico.html" /usr/share/nginx/html/index.html
EXPOSE 80
