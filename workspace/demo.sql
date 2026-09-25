-- demo.sql — recorrido por el SQL que soporta el DBMS.
-- ▶ ANALIZAR muestra tokens, árbol sintáctico (ANTLR) y problemas.

CREATE DATABASE IF NOT EXISTS tienda;
USE tienda;

CREATE TABLE cliente (
    id      INT PRIMARY KEY,
    nombre  VARCHAR(50) NOT NULL,
    email   VARCHAR(80) UNIQUE,
    alta    DATE DEFAULT '2024-01-01',
    activo  BOOLEAN DEFAULT TRUE,
    CONSTRAINT ck_id CHECK (id > 0)
);

CREATE TABLE pedido (
    id          INT,
    cliente_id  INT NOT NULL,
    total       FLOAT DEFAULT 0,
    CONSTRAINT pk_pedido PRIMARY KEY (id),
    CONSTRAINT fk_cliente FOREIGN KEY (cliente_id) REFERENCES cliente (id),
    CHECK (total >= 0)
);

CREATE INDEX idx_pedido_cliente ON pedido (cliente_id);

INSERT INTO cliente (id, nombre, email) VALUES
    (1, 'Ana', 'ana@mail.com'),
    (2, 'Luis', 'luis@mail.com'),
    (3, 'O''Brien', NULL);

INSERT INTO pedido VALUES (10, 1, 250.5), (11, 1, 99.9), (12, 2, 15);

UPDATE cliente SET activo = FALSE WHERE email IS NULL;
DELETE FROM pedido WHERE total < 20;

SELECT c.nombre, COUNT(*) AS pedidos, SUM(p.total) AS gastado
FROM cliente c
LEFT JOIN pedido p ON p.cliente_id = c.id
WHERE c.activo = TRUE AND c.nombre LIKE 'A%'
GROUP BY c.nombre
HAVING COUNT(*) >= 1
ORDER BY gastado DESC
LIMIT 10;

ALTER TABLE cliente ADD COLUMN telefono VARCHAR(15);
SHOW TABLES;
DESCRIBE cliente;
