-- errores_sintaxis.sql — cada línea tiene un error léxico o sintáctico
-- intencional; ▶ ANALIZAR los lista en PROBLEMAS con línea y columna.

SELECT FROM cliente;
CREATE TABLE t (id INT,);
UPDATE cliente SET nombre 'x';
CREATE TABLA x (id INT);
SELECT @ FROM cliente;
INSERT INTO t VALUES (1 2);
SELECT * FROM cliente WHERE nombre = 'sin cerrar
